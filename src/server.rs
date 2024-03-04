use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
};

use walkdir::WalkDir;

use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{transport::Server as tServer, Status};
use tonic::{Request, Response};

mod model {
    include!("model.rs");
}

use self::model::{Directory, FileHasher, CHUNK_SIZE};

use self::pb_proto::{
    lan_doh_server, lan_doh_server::LanDoh, FileMetaData, GetDirectoryRequest,
    GetDirectoryResponse, GetFileRequest, GetFileResponse,
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

        let path = PathBuf::from(r.path);

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

        let size = path.metadata()?.len();

        let mut start_bytes: u64;

        match r.from_bytes {
            Some(start) => {
                start_bytes = start;
            }
            None => {
                start_bytes = 0;
            }
        }

        let (tx, rx): (
            Sender<Result<GetFileResponse, Status>>,
            Receiver<Result<GetFileResponse, Status>>,
        ) = mpsc::channel(128);

        tokio::spawn(async move {
            let mut source_file: File;
            match File::open(path.clone()) {
                Ok(f) => {
                    source_file = f;
                }
                Err(err) => {
                    println!("ERROR: failed to update stream client: {:?}", err);
                    let e = Err(Status::internal(format!("{}", err)));
                    let _ = tx.send(e.clone()).await;
                    return e;
                }
            };

            loop {
                if size <= start_bytes {
                    return Ok(GetFileResponse { chunk: vec![0, 0] });
                }

                let chunk: usize;
                if size - start_bytes >= CHUNK_SIZE as u64 {
                    chunk = CHUNK_SIZE;
                } else {
                    chunk = (size - start_bytes) as usize;
                }
                let mut buf = vec![0; chunk];

                source_file.seek(SeekFrom::Start(start_bytes))?;
                let mut reader = BufReader::new(&source_file);
                reader.read_exact(&mut buf)?;

                if buf.len() == 0 {
                    return Ok(GetFileResponse { chunk: vec![0, 0] });
                }

                let resp = GetFileResponse { chunk: buf };

                match tx.send(Ok(resp)).await {
                    Ok(_) => {
                        start_bytes += CHUNK_SIZE as u64;
                    }
                    Err(err) => {
                        println!("ERROR: failed to update stream client: {:?}", err);
                        let e = Err(Status::internal(format!("{}", err)));
                        let _ = tx.send(e.clone()).await;
                        return e;
                    }
                }
            }
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
