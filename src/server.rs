use std::{
    error::Error,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    net::SocketAddr,
    os::unix::fs::MetadataExt,
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

#[derive(Debug)]
pub struct Server {
    directories: Vec<String>,
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
}

impl Default for Server {
    fn default() -> Self {
        Server {
            directories: vec![".".to_string()],
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
        if !self.directories.contains(&r.name) {
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

        let mut files: Vec<String> = vec![];

        for f in WalkDir::new(&path) {
            let e = f.unwrap();

            if e.metadata().unwrap().is_dir() {
                continue;
            }

            files.push(String::from(e.path().to_str().unwrap()));
        }

        Ok(Response::new(GetDirectoryResponse { files: files }))
    }

    async fn get_file(
        &self,
        request: Request<GetFileRequest>,
    ) -> Result<Response<Self::GetFileStream>, Status> {
        let r = request.into_inner();

        let path = PathBuf::from(r.path);

        let mut root_path = path.clone();

        while root_path.has_root() {
            match root_path.parent() {
                Some(p) => root_path = p.to_path_buf(),
                None => {
                    if !self
                        .directories
                        .contains(&root_path.to_str().unwrap().to_string())
                    {
                        return Err(Status::invalid_argument(format!(
                            "invalid item: {:?}",
                            &path
                        )));
                    }
                }
            }
        }

        if !path.exists() {
            return Err(Status::invalid_argument(format!(
                "item is marked for sharing but could not be found: {:?}",
                &path
            )));
        }

        let size = path.metadata()?.size();

        let mut start_bytes: u64;

        match r.from_bytes {
            Some(start) => {
                start_bytes = start as u64;
            }
            None => {
                start_bytes = 0;
            }
        }

        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let chunk_size: usize = 1024 * 4;
            loop {
                let mut source_file: File;
                match File::open(path.clone()) {
                    Ok(f) => {
                        source_file = f;
                    }
                    Err(err) => {
                        println!("ERROR: failed to update stream client: {:?}", err);
                        let e = Err(Status::internal(format!("{}", err)));
                        tx.send(e.clone()).await;
                        return e;
                        // tx.send(err);
                    }
                };
                let chunk: usize;
                if size - start_bytes >= chunk_size as u64 {
                    chunk = chunk_size;
                } else if size - start_bytes > 0 {
                    chunk = (size - start_bytes) as usize;
                } else {
                    let resp = GetFileResponse {
                        file_response: Some(frep::MetaData(FileMetaData {
                            file_size: size as u64,
                            hash: "abc".to_string(),
                        })),
                    };
                    tx.send(Ok(resp.clone())).await;
                    return Ok(resp);
                }

                source_file.seek(SeekFrom::Start(start_bytes));

                let mut buf = source_file.take(chunk as u64).bytes();

                let resp = GetFileResponse {
                    file_response: Some(frep::Chunk(chunk as u32)),
                };

                match tx.send(Ok(resp)).await {
                    Ok(_) => {
                        start_bytes += chunk_size as u64;
                    }
                    Err(err) => {
                        println!("ERROR: failed to update stream client: {:?}", err);
                        let e = Err(Status::internal(format!("{}", err)));
                        tx.send(e.clone());
                        return e;
                    }
                }
            }
        });

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
    let sv = Server {
        directories: vec!["testdir".to_string()],
    };

    sv.serve(addr).await?;

    Ok(())
}
