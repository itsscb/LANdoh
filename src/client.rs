use std::{
    error::Error,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use landoh::landoh::{landoh_client::LandohClient, DirectoryRequest};
use tokio_stream::StreamExt;

mod landoh_proto {
    include!("landoh.rs");
}

pub struct Client {
    share_path: String,
}

impl Client {
    pub fn new(share_path: String) -> Client {
        Client { share_path }
    }
    pub async fn get_directory(&self, name: String, addr: String) -> Result<(), Box<dyn Error>> {
        dbg!(
            "LANdoh::get_directory::{:?}::{} (CLIENT / CONNECTING)",
            &addr,
            &name
        );
        let mut client = LandohClient::connect(addr).await?;

        let message = DirectoryRequest { name: name };

        let request = tonic::Request::new(message);

        let mut stream = client.get_directory(request).await.unwrap().into_inner();

        while let Some(resp) = stream.next().await {
            let r = resp.unwrap();
            dbg!("LANdoh::get_directory::{} (CLIENT / RECEIVED)", &r.path);

            let path: String;

            if r.path.starts_with("./") {
                path = r.path.replace("./", "");
            } else {
                path = r.path;
            }

            let path = PathBuf::from(&self.share_path).join(path);
            let parent = path.parent().unwrap();

            fs::create_dir_all(&parent)?;

            let mut file = File::create(path)?;
            let c: Vec<u8> = r.chunk.into_iter().map(|i| i as u8).collect();
            file.write_all(&c)?;
        }

        Ok(())
    }
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "http://127.0.0.1:9001".to_string();
    let c = Client::new("testdestination".to_string());
    c.get_directory("./testdir".to_string(), addr).await?;

    Ok(())
}
