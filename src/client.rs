use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

// use landoh::landoh::{landoh_client::LandohClient, DirectoryRequest};
use tokio_stream::StreamExt;

// mod landoh_proto {
//     include!("landoh.rs");
// }

use pb_proto::{lan_doh_client, GetDirectoryRequest};
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
    pub async fn get_directory(&self, name: String, addr: String) -> Result<(), Box<dyn Error>> {
        println!(
            "LANdoh::get_directory::{:?}::{} (CLIENT / CONNECTING)",
            &addr, &name
        );
        let mut client = lan_doh_client::LanDohClient::connect(addr).await?;

        let message = GetDirectoryRequest { name: name };

        let request = tonic::Request::new(message);

        let response = client.get_directory(request).await.unwrap().into_inner();

        println!("file: {:?}", response.files);
        // let mut curr_file = "".to_string();
        // while let Some(resp) = stream.next().await {
        //     let r = resp.unwrap();

        //     let path: String;
        //     let print = curr_file != r.path;
        //     curr_file = r.path.clone();

        //     if r.path.starts_with("./") {
        //         path = r.path.replace("./", "");
        //     } else {
        //         path = r.path;
        //     }

        //     if print {
        //         println!("receiving: {}", &path);
        //     }

        //     let path = PathBuf::from(&self.share_path).join(path);
        //     let parent = path.parent().unwrap();

        //     fs::create_dir_all(&parent)?;

        //     let mut file = OpenOptions::new();
        //     if Path::new(&path).exists() {
        //         file.append(true);
        //     } else {
        //         file.write(true);
        //     }

        //     let mut file = file.create(true).open(path)?;

        //     let c: Vec<u8> = r.chunk.into_iter().map(|i| i as u8).collect();
        //     file.write_all(&c)?;
        // }

        Ok(())
    }
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "http://127.0.0.1:9001".to_string();
    let c = Client::new("testdestination".to_string());
    c.get_directory("testdir".to_string(), addr).await?;

    Ok(())
}
