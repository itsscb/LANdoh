use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use tokio_stream::StreamExt;

use pb_proto::{lan_doh_client, FileMetaData, GetDirectoryRequest, GetFileRequest};
mod pb_proto {
    include!("pb.rs");
}

pub struct Client {
    share_path: String,
}

impl Client {
    pub fn new(share_path: String) -> Client {
        Client { share_path }
    }

    pub async fn get_file(&self, addr: String, file: FileMetaData) -> Result<(), Box<dyn Error>> {
        println!("requesting '{}' from {}", &file.path, &addr);
        let mut client = lan_doh_client::LanDohClient::connect(addr).await?;

        let tmp_path = file.path.clone();

        let mut from_bytes: u64 = 0;
        let path = PathBuf::from(&self.share_path).join(&file.path);

        let mut dest_file = OpenOptions::new();
        if path.exists() {
            dest_file.append(true);

            let size = path.metadata()?.len();

            if size < file.file_size {
                from_bytes = size;
            } else {
                println!("already got '{}'", &file.path);
                return Ok(());
            }
        } else {
            dest_file.write(true);
        }
        let message = GetFileRequest {
            path: tmp_path,
            from_bytes: Some(from_bytes),
        };

        let request = tonic::Request::new(message);

        let mut stream = client.get_file(request).await.unwrap().into_inner();

        let mut written: u64 = 0;
        let path = path.clone();
        let parent = path.parent().unwrap();

        fs::create_dir_all(&parent)?;

        let mut dest_file = dest_file.create(true).open(&path)?;

        while let Some(resp) = stream.next().await {
            match resp {
                Ok(p) => {
                    let c: Vec<u8> = p.chunk.into_iter().map(|i| i as u8).collect();
                    for i in &c {
                        written += *i as u64;
                    }
                    dest_file.write_all(&c)?;
                }
                Err(err) => {
                    println!("{:?}", err);
                }
            }
        }
        println!("file: {:?}, received: {:?}", path, written);
        Ok(())
    }

    pub async fn get_directory(
        &self,
        name: String,
        addr: String,
    ) -> Result<Vec<FileMetaData>, Box<dyn Error>> {
        println!(
            "LANdoh::get_directory::{:?}::{} (CLIENT / CONNECTING)",
            &addr, &name
        );
        let mut client = lan_doh_client::LanDohClient::connect(addr).await?;

        let message = GetDirectoryRequest { name: name };

        let request = tonic::Request::new(message);

        let response = client.get_directory(request).await.unwrap().into_inner();

        println!("file: {:?}", response.files);

        Ok(response.files)
    }
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "http://127.0.0.1:9001".to_string();
    let c = Client::new("testdestination".to_string());
    let files = c
        .get_directory("testdir".to_string(), addr.clone())
        .await
        .unwrap();

    for f in files.clone() {
        let _ = c.get_file(addr.clone(), f).await;
    }

    Ok(())
}
