use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::{error::Error, pin::Pin};

use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_stream::{wrappers::ReceiverStream, Stream};
use tonic::{Request, Response, Status};
use walkdir::WalkDir;

use landoh::{DirectoryRequest, FileResponse};

pub mod landoh;
pub mod server;

type ResponseStream = Pin<Box<dyn Stream<Item = Result<FileResponse, Status>> + Send>>;
type DirectoryResult<T> = Result<Response<T>, Status>;

#[derive(Debug, Default)]
pub struct MyServer;

async fn send_dir<T>(
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
                chunk: buf.into_iter().map(|v| v as u32).collect(), // chunk: vec![]
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

#[tonic::async_trait]
impl landoh::landoh_server::Landoh for MyServer {
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
            let _ = send_dir(path.to_str().unwrap(), tx).await;
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(
            Box::pin(output_stream) as Self::GetDirectoryStream
        ))
    }
}
