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

use self::model::{Directory, CHUNK_SIZE};

use self::pb_proto::{
    get_file_response::FileResponse, lan_doh_server, lan_doh_server::LanDoh, FileMetaData,
    GetDirectoryRequest, GetDirectoryResponse, GetFileRequest, GetFileResponse,
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

    async fn get_directory(
        &self,
        request: Request<GetDirectoryRequest>,
    ) -> Result<Response<GetDirectoryResponse>, Status> {
        let r = request.into_inner();
        let path = PathBuf::from(r.name.clone());
        if !self.dir_is_shared(&r.name) {
            return Err(Status::invalid_argument(format!(
                "invalid item: {}",
                r.name
            )));
        }

        if !&path.exists() {
            return Err(Status::not_found(format!(
                "item is marked for sharing but could not be found: {}",
                r.name
            )));
        }

        let mut files: Vec<FileMetaData> = vec![];

        for f in WalkDir::new(&path) {
            let e = f.unwrap();

            let m = e.metadata().unwrap();
            if m.is_dir() {
                continue;
            }

            files.push(FileMetaData {
                file_size: m.len(),
                hash: "none".to_string(),
                // hash: file_hash(&PathBuf::from(e.path())).unwrap_or("none".to_string()),
                path: String::from(e.path().to_str().unwrap()),
            });
        }

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
            if d.contains_partial_path(path.to_str()) {
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
            // let mut source_file: File;
            // match File::open(path.clone()) {
            //     Ok(f) => {
            //         source_file = f;
            //     }
            //     Err(err) => {
            //         println!("ERROR: failed to update stream client: {:?}", err);
            //         let e = Err(Status::internal(format!("{}", err)));
            //         let _ = tx.send(e.clone()).await;
            //         return e;
            //     }
            // };

            // loop {
            //     if size <= start_bytes {
            //         return Ok(GetFileResponse { chunk: vec![0, 0] });
            //     }

            //     let chunk: usize;
            //     if size - start_bytes >= CHUNK_SIZE as u64 {
            //         chunk = CHUNK_SIZE;
            //     } else {
            //         chunk = (size - start_bytes) as usize;
            //     }
            //     let mut buf = vec![0; chunk];

            //     source_file.seek(SeekFrom::Start(start_bytes))?;
            //     let mut reader = BufReader::new(&source_file);
            //     reader.read_exact(&mut buf)?;

            //     if buf.len() == 0 {
            //         return Ok(GetFileResponse { chunk: vec![0, 0] });
            //     }

            //     let resp = GetFileResponse { chunk: buf };

            //     match tx.send(Ok(resp)).await {
            //         Ok(_) => {
            //             start_bytes += CHUNK_SIZE as u64;
            //         }
            //         Err(err) => {
            //             println!("ERROR: failed to update stream client: {:?}", err);
            //             let e = Err(Status::internal(format!("{}", err)));
            //             let _ = tx.send(e.clone()).await;
            //             return e;
            //         }
            //     }
            // }
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
    let sv = Server {
        directories: vec![Directory {
            name: "testdir".to_string(),
            paths: vec!["testdir".to_string()],
        }],
    };

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
