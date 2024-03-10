use std::{error::Error, fs::File, io::Read, net::SocketAddr, path::PathBuf, pin::Pin};

use data_encoding::HEXUPPER;
use ring::digest::{Context, SHA256};

use walkdir::WalkDir;

use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{transport::Server as tServer, Status};
use tonic::{Request, Response};

mod model {
    include!("model.rs");
}

use self::model::{contains_partial_path, CHUNK_SIZE};

use self::pb_proto::{
    get_file_response::FileResponse, lan_doh_server, lan_doh_server::LanDoh, Directory,
    FileMetaData, GetDirectoryRequest, GetDirectoryResponse, GetFileRequest, GetFileResponse,
    ListDirectoriesRequest, ListDirectoriesResponse,
};

mod pb_proto {
    include!("pb.rs");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("pb_descriptor");
}

#[derive(Debug)]
pub struct Server {
    directories: Vec<Directory>,
}

impl Server {
    pub fn new(directories: Vec<String>) -> Self {
        Server {
            directories: directories
                .iter()
                .map(|d| Directory {
                    name: d.to_string(),
                    paths: vec![d.to_string()],
                })
                .collect(),
        }
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

    pub fn dir_is_shared(&self, name: &String) -> bool {
        for d in &self.directories {
            if d.name == *name {
                return true;
            }
        }
        false
    }

    pub fn get_dir(&self, name: &String) -> Option<&Directory> {
        self.directories.iter().find(|d| &d.name == name)
    }

    pub fn add_shared_dir(
        &mut self,
        name: String,
        paths: Vec<String>,
    ) -> Result<(), Box<dyn Error>> {
        let mut existing_paths = vec![];

        for pa in paths {
            let p = PathBuf::from(&pa);
            if p.exists() && p.metadata()?.is_dir() {
                existing_paths.push(pa);
            }
        }
        let _ = &self.directories.push(Directory {
            name: name.to_string(),
            paths: existing_paths,
        });
        Ok(())
    }

    pub fn remove_shared_dir(&mut self, name: String) {
        self.directories.retain(|d| d.name != name);
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            directories: vec![Directory {
                name: "root".to_string(),
                paths: vec![".".to_string()],
            }],
        }
    }
}

#[tonic::async_trait]
impl LanDoh for Server {
    type GetFileStream = Pin<Box<dyn Stream<Item = Result<GetFileResponse, Status>> + Send>>;
    async fn list_directories(
        &self,
        _request: Request<ListDirectoriesRequest>,
    ) -> Result<Response<ListDirectoriesResponse>, Status> {
        Ok(Response::new(ListDirectoriesResponse {
            dirs: self.directories.iter().map(|d| d.clone()).collect(),
        }))
    }

    async fn get_directory(
        &self,
        request: Request<GetDirectoryRequest>,
    ) -> Result<Response<GetDirectoryResponse>, Status> {
        let r = request.into_inner();

        let dir_res = self.get_dir(&r.name);

        if dir_res == None {
            return Err(Status::invalid_argument(format!(
                "invalid item: {}",
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

                files.push(FileMetaData {
                    file_size: m.len(),
                    hash: "none".to_string(),
                    path: String::from(e.path().to_str().unwrap()),
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

        let path = PathBuf::from(r.path.clone());

        let mut shared_dir = false;
        for d in &self.directories {
            if contains_partial_path(path.to_str(), &d.paths) {
                shared_dir = true;
            }
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
            send_file(r.path.as_str(), tx).await;
        });

        let output_stream: ReceiverStream<Result<GetFileResponse, Status>> =
            ReceiverStream::new(rx);

        Ok(Response::new(Box::pin(output_stream) as Self::GetFileStream))
    }
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr: SocketAddr = "0.0.0.0:9001".parse()?;
    let mut sv = Server::default();
    sv.add_shared_dir("test".to_string(), vec!["testdir".to_string()])
        .unwrap();

    sv.serve(addr).await?;

    Ok(())
}

pub async fn send_file(path: &str, tx: Sender<Result<GetFileResponse, Status>>) {
    let mut reader: File = match File::open(&path) {
        Ok(f) => f,
        Err(err) => {
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

#[test]
fn test_server_add_shared_dir() {
    let mut s = Server::default();
    let _ = s.add_shared_dir("test".to_string(), vec!["testdir".to_string()]);

    assert!(s
        .directories
        .iter()
        .any(|i| i.name == "root".to_string() && i.paths.iter().any(|p| *p == ".".to_string())));
    assert!(s.directories.iter().any(
        |i| i.name == "test".to_string() && i.paths.iter().any(|p| *p == "testdir".to_string())
    ));

    let _ = s.remove_shared_dir("test".to_string());

    let index = s
        .directories
        .iter()
        .position(|d| d.name == "test".to_string());
    assert_eq!(index, None);
}
