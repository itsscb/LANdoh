use std::{
    error::Error,
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

type ResponseStream = Pin<Box<dyn Stream<Item = Result<FileResponse, Status>> + Send>>;
type DirectoryResult<T> = Result<Response<T>, Status>;

#[derive(Debug, Default)]
pub struct Server;

#[tonic::async_trait]
impl landoh_server::Landoh for Server {
    type GetDirectoryStream = ResponseStream;

    async fn get_directory(
        &self,
        request: Request<DirectoryRequest>,
    ) -> DirectoryResult<Self::GetDirectoryStream> {
        println!("LANdoh::get_directory");
        println!("\tclient connected from: {:?}", request.remote_addr());

        let path = PathBuf::from(request.into_inner().name);

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let _ = Server::send_dir(&Self, path.to_str().unwrap(), tx).await;
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::GetDirectoryStream
        ))
    }
}

impl Server {
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
