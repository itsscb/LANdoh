use std::{
    error::Error,
    fmt::format,
    fs::File,
    io::{BufReader, Read},
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
};

use tokio::sync::mpsc::{self, Sender};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{transport::Server as tServer, Status};
use tonic::{Request, Response};

use walkdir::WalkDir;

use landoh_proto::{landoh_server, DirectoryRequest, FileResponse};

mod landoh_proto {
    include!("landoh.rs");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("landoh_descriptor");
}

use pb_proto::{
    get_file_response::FileResponse as frep, lan_doh_server, FileMetaData, GetDirectoryRequest,
    GetDirectoryResponse, GetFileRequest, GetFileResponse,
};

use self::pb_proto::lan_doh_server::LanDoh;
mod pb_proto {
    include!("pb.rs");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("pb_descriptor");
}

#[derive(Debug, Default)]
pub struct Server;

impl Server {
    pub async fn serve(&self, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(pb_proto::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        tServer::builder()
            .add_service(lan_doh_server::LanDohServer::new(Self))
            .add_service(reflection_service)
            .serve(addr)
            .await?;
        Ok(())
    }
}

#[tonic::async_trait]
impl LanDoh for Server {
    type GetFileStream = Pin<Box<dyn Stream<Item = Result<GetFileResponse, Status>> + Send>>;

    async fn get_directory(
        &self,
        request: Request<GetDirectoryRequest>,
    ) -> Result<Response<GetDirectoryResponse>, Status> {
        let r = request.into_inner();
        let path = PathBuf::from(r.name.clone());
        if !path.exists() {
            return Err(Status::not_found(format!(
                "directory not found: {}",
                r.name
            )));
        }

        let mut files: Vec<String> = vec![];

        for f in WalkDir::new(&path) {
            let e = f.unwrap();

            if e.metadata().unwrap().is_dir() {
                continue;
            }

            files.push(String::from(e.path().to_str().unwrap()));
        }

        // unimplemented!("{}", "get_directory");
        Ok(Response::new(GetDirectoryResponse { path: files }))
    }

    async fn get_file(
        &self,
        request: Request<GetFileRequest>,
    ) -> Result<Response<Self::GetFileStream>, Status> {
        unimplemented!("{}", "get_file");
        // Ok(Response::new(Box::pin(GetFileResponse {
        //     file_response: Some(frep::MetaData(FileMetaData {
        //         file_size: 20 as u32,
        //         hash: "blub".to_string(),
        //     })),
        // }))) as Self::GetFileStream
        let (tx, rx) = mpsc::channel(128);

        let output_stream = ReceiverStream::new(rx);

        Ok(Response::new(Box::pin(output_stream) as Self::GetFileStream))
    }
}

type FileResponseStream = Pin<Box<dyn Stream<Item = Result<FileResponse, Status>> + Send>>;
type DirectoryResult<T> = Result<Response<T>, Status>;

#[derive(Debug, Default)]
pub struct ServerOld;

#[tonic::async_trait]
impl landoh_server::Landoh for ServerOld {
    type GetDirectoryStream = FileResponseStream;

    async fn get_directory(
        &self,
        request: Request<DirectoryRequest>,
    ) -> DirectoryResult<Self::GetDirectoryStream> {
        println!("LANdoh::get_directory");
        println!("\tclient connected from: {:?}", request.remote_addr());

        let path = PathBuf::from(request.into_inner().name);

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let _ = ServerOld::send_dir(&Self, path.to_str().unwrap(), tx).await;
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::GetDirectoryStream
        ))
    }
}

impl ServerOld {
    pub async fn serve(&self, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(landoh_proto::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        tServer::builder()
            .add_service(landoh_server::LandohServer::new(Self))
            .add_service(reflection_service)
            .serve(addr)
            .await?;
        Ok(())
    }
    async fn send_dir<T>(
        &self,
        path: &str,
        tx: Sender<Result<FileResponse, T>>,
    ) -> Result<(), Box<dyn Error>> {
        let path = PathBuf::from(path);
        let chunk_size: usize = 1024 * 4;

        for entry in WalkDir::new(&path) {
            let e = entry.unwrap();
            dbg!("sending: {}", e.path());
            if e.metadata().unwrap().is_dir() {
                continue;
            }

            let sf = File::open(e.path()).unwrap();
            let mut reader = BufReader::new(&sf);

            let size = e.metadata().unwrap().len();
            let mut send: u64 = 0;

            loop {
                let chunk: usize;
                if size - send >= chunk_size as u64 {
                    chunk = chunk_size;
                } else {
                    chunk = (size - send) as usize;
                }
                let mut buf = vec![0; chunk];
                reader.read_exact(&mut buf)?;

                if buf.len() == 0 {
                    break;
                }

                let resp = FileResponse {
                    path: e.path().display().to_string(),
                    chunk: buf.into_iter().map(|v| v as u32).collect(),
                };

                match tx.send(Ok(resp)).await {
                    Ok(_) => {
                        send += chunk as u64;
                    }
                    Err(err) => {
                        println!("ERROR: failed to update stream client: {:?}", err);
                        break;
                    }
                }
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr: SocketAddr = "0.0.0.0:9001".parse()?;
    let sv = Server::default();

    sv.serve(addr).await?;

    Ok(())
}
