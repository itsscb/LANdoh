use std::sync::Arc;
use std::{error::Error, fs::File, io::Read, net::SocketAddr, path::PathBuf, pin::Pin};

use data_encoding::HEXUPPER;
use log::{error, warn};
use ring::digest::{Context, SHA256};

use walkdir::WalkDir;

use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{transport::Server as tServer, Status};
use tonic::{Request, Response};

use crate::app::Config;
use crate::model::CHUNK_SIZE;

use self::pb_proto::{
    get_file_response::FileResponse, lan_doh_server, lan_doh_server::LanDoh, FileMetaData,
    GetDirectoryRequest, GetDirectoryResponse, GetFileRequest, GetFileResponse,
    ListDirectoriesRequest, ListDirectoriesResponse, HealthzRequest, HealthzResponse
};

pub use self::pb_proto::Directory;

use crate::shorten_path;

// mod pb_proto {
//     include!("pb.rs");

//     pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
//         tonic::include_file_descriptor_set!("pb_descriptor");
// }

mod pb_proto {
    tonic::include_proto!("pb");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("pb_descriptor");
}

#[derive(Debug)]
pub struct Server {
    config: Arc<tokio::sync::Mutex<Config>>,
}

impl Server {
    pub fn new(config: Arc<tokio::sync::Mutex<Config>>) -> Self {
        Server { config }
    }

    pub async fn serve(self, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(pb_proto::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        tServer::builder()
            .add_service(lan_doh_server::LanDohServer::new(self))
            .add_service(reflection_service)
            .serve(addr)
            .await?;
        Ok(())
    }

    pub async fn get_dir(&self, name: &String) -> Option<Directory> {
        match self
            .config
            .lock()
            .await
            .shared_directories
            .iter()
            .find(|d| &d.name == name)
        {
            Some(d) => Some(d.clone()),
            None => None,
        }
    }
}

#[tonic::async_trait]
impl LanDoh for Server {
    type GetFileStream = Pin<Box<dyn Stream<Item = Result<GetFileResponse, Status>> + Send>>;
    
    async fn healthz(
        &self,
        _request: tonic::Request<HealthzRequest>,
    ) -> Result<tonic::Response<HealthzResponse>, tonic::Status> {
        
        Ok(tonic::Response::new(HealthzResponse{
            broadcaster: true, 
            event_listener: true,
            address: "".to_string(),
            id: "".to_string(),
            nickname: "".to_string()
        }))
    }
    
    async fn list_directories(
        &self,
        _request: Request<ListDirectoriesRequest>,
    ) -> Result<Response<ListDirectoriesResponse>, Status> {
        Ok(Response::new(ListDirectoriesResponse {
            dirs: self
                .config
                .lock()
                .await
                .shared_directories
                .iter()
                .map(|d| Directory {
                    name: d.name.clone(),
                    paths: d
                        .paths
                        .iter()
                        .map(|p| shorten_path(d.name.clone(), p.clone()))
                        .collect(),
                })
                .collect(),
        }))
    }

    async fn get_directory(
        &self,
        request: Request<GetDirectoryRequest>,
    ) -> Result<Response<GetDirectoryResponse>, Status> {
        let r = request.into_inner();

        let dir_res = self.get_dir(&r.name).await;

        if dir_res == None {
            warn!("{:?} / {:?}", r, self);
            return Err(Status::invalid_argument(format!(
                "GetDir: invalid item: {}",
                r.name
            )));
        }

        let dir = dir_res.unwrap();

        let mut files: Vec<FileMetaData> = vec![];
        dir.paths.iter().map(|p| PathBuf::from(p)).for_each(|path| {
            if !&path.exists() {
                return;
            }

            for f in WalkDir::new(&path) {
                let e = f.unwrap();

                let m = e.metadata().unwrap();
                if m.is_dir() {
                    continue;
                }

                let p = shorten_path(dir.name.clone(), String::from(e.path().to_str().unwrap()));

                files.push(FileMetaData {
                    file_size: m.len(),
                    hash: "none".to_string(),
                    path: p,
                });
            }
        });

        Ok(Response::new(GetDirectoryResponse { files: files }))
    }

    async fn get_file(
        &self,
        request: Request<GetFileRequest>,
    ) -> Result<Response<Self::GetFileStream>, Status> {
        let r = request.into_inner();

        let mut path = PathBuf::from(r.path.clone());

        let dir_name: PathBuf = path.iter().take(1).collect();

        let mut shared_dir = false;

        {
            self.config
                .lock()
                .await
                .shared_directories
                .iter()
                .for_each(|d| {
                    if d.name == dir_name.to_str().unwrap().to_string() {
                        shared_dir = true;
                        path = PathBuf::from(d.paths[0].clone())
                            .parent()
                            .unwrap()
                            .join(&path);
                    }
                });
        }

        if !shared_dir {
            return Err(Status::invalid_argument(format!(
                "invalid item: {:?}",
                &path
            )));
        }

        if !path.exists() {
            return Err(Status::invalid_argument(format!(
                "item is marked for sharing but could not be found: {:?}",
                &path
            )));
        }

        let (tx, rx): (
            Sender<Result<GetFileResponse, Status>>,
            Receiver<Result<GetFileResponse, Status>>,
        ) = mpsc::channel(128);

        tokio::spawn(async move {
            send_file(&path.to_str().unwrap(), tx).await;
        });

        let output_stream: ReceiverStream<Result<GetFileResponse, Status>> =
            ReceiverStream::new(rx);

        Ok(Response::new(Box::pin(output_stream) as Self::GetFileStream))
    }
}

pub async fn send_file(path: &str, tx: Sender<Result<GetFileResponse, Status>>) {
    let mut reader: File = match File::open(&path) {
        Ok(f) => f,
        Err(err) => {
            error!("{:?}: {:?}", &path, err);
            let e = Err(Status::internal(format!(
                "ERROR: failed to open file: {:?}",
                err
            )));
            let _ = tx.send(e.clone()).await;
            return;
        }
    };
    let mut context = Context::new(&SHA256);
    let size = reader.metadata().unwrap().len();
    let mut start_bytes = 0;
    loop {
        let chunk: usize;
        if size - start_bytes >= CHUNK_SIZE as u64 {
            chunk = CHUNK_SIZE;
        } else {
            chunk = (size - start_bytes) as usize;
        }
        let mut buffer = vec![0; chunk];
        let count = match reader.read(&mut buffer) {
            Ok(r) => r,
            Err(err) => {
                let _ = tx
                    .send(Err(Status::internal(format!(
                        "failed to read file: {}",
                        err
                    ))))
                    .await;
                return;
            }
        };
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
        let _ = tx
            .send(Ok(GetFileResponse {
                file_response: Some(FileResponse::Chunk(buffer)),
            }))
            .await;
        start_bytes += count as u64;
    }
    let hash = HEXUPPER.encode(context.finish().as_ref());
    let _ = tx
        .send(Ok(GetFileResponse {
            file_response: Some(FileResponse::Meta(FileMetaData {
                file_size: size as u64,
                path: path.to_string(),
                hash: hash,
            })),
        }))
        .await;
}
