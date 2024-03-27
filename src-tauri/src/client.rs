use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Arc,
};

use data_encoding::HEXUPPER;
use ring::digest::{Context, SHA256};
use tokio_stream::StreamExt;

use super::pb::{
    get_file_response::FileResponse, lan_doh_client, FileMetaData, GetDirectoryRequest,
    GetFileRequest, ListDirectoriesRequest,
};

use log::{error, info};

pub struct Client {
    share_path: String,
}

impl Client {
    pub fn new(share_path: String) -> Client {
        Client { share_path }
    }

    pub async fn get_all_files(
        self: Arc<Self>,
        addr: String,
        files: Vec<FileMetaData>,
    ) -> Result<(), Box<dyn Error>> {
        let mut handles = vec![];

        let addr = Arc::new(addr);
        let client = self;
        for f in files {
            let a = addr.clone();
            let c = client.clone();
            handles.push(tokio::spawn(async move {
                match c.get_file(a.to_string(), f).await {
                    Ok(_) => {}
                    Err(err) => {
                        error!("{:?}", err);
                    }
                };
            }));
        }

        for h in handles {
            let _ = h.await;
        }

        Ok(())
    }

    pub async fn get_file(&self, addr: String, file: FileMetaData) -> Result<(), Box<dyn Error>> {
        info!("requesting '{}' from {}", &file.path, &addr);
        let mut client = lan_doh_client::LanDohClient::connect(addr).await?;

        let tmp_path = file.path.clone();

        let path = PathBuf::from(&self.share_path).join(&file.path);

        let mut dest_file = OpenOptions::new();
        if path.exists() {
            return Err(format!("file already exists: {:?}", path).into());
        } else {
            dest_file.write(true);
        }
        let message = GetFileRequest { path: tmp_path };

        let request = tonic::Request::new(message);

        let mut stream = client.get_file(request).await.unwrap().into_inner();

        let mut written: u64 = 0;
        let path = path.clone();
        let parent = path.parent().unwrap();

        // TODO:
        // Convert absolute path to relative path
        // before creating dir tree and saving files

        fs::create_dir_all(&parent)?;

        let mut dest_file = dest_file.create(true).open(&path)?;
        let mut context = Context::new(&SHA256);
        let mut fileresp: FileMetaData = FileMetaData {
            path: "".to_string(),
            file_size: 0,
            hash: "".to_string(),
        };
        while let Some(resp) = stream.next().await {
            match resp {
                Ok(p) => {
                    let r = match p.file_response {
                        Some(r) => r,
                        None => {
                            break;
                        }
                    };

                    match r {
                        FileResponse::Chunk(c) => {
                            for i in &c {
                                written += *i as u64;
                            }
                            context.update(&c.clone());
                            dest_file.write_all(&c)?;
                        }
                        FileResponse::Meta(m) => {
                            fileresp = m;
                        }
                    }
                }
                Err(err) => {
                    error!("{:?}", err);
                }
            }
        }

        let hash = HEXUPPER.encode(context.finish().as_ref());

        info!(
            "file: {:?}, received: {:?}, valid: {:?}",
            path,
            written,
            fileresp.hash == hash
        );
        Ok(())
    }

    pub async fn list_directories(&self, addr: String) -> Result<(), Box<dyn Error>> {
        let mut client = lan_doh_client::LanDohClient::connect(addr).await?;
        let _ = client
            .list_directories(tonic::Request::new(ListDirectoriesRequest {}))
            .await
            .unwrap()
            .into_inner();

        Ok(())
    }

    pub async fn get_directory(
        &self,
        name: String,
        addr: String,
    ) -> Result<Vec<FileMetaData>, Box<dyn Error>> {
        let mut client = lan_doh_client::LanDohClient::connect(addr).await?;

        let message = GetDirectoryRequest { name: name };

        let request = tonic::Request::new(message);

        let response = client.get_directory(request).await.unwrap().into_inner();

        Ok(response.files)
    }
}

//
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     use clap::{Parser, Subcommand};

//     #[derive(Parser)]
//     struct Cli {
//         #[arg(short, long)]
//         address: Option<String>,
//         #[arg(short, long)]
//         destination: Option<String>,
//         #[command(subcommand)]
//         command: Option<Commands>,
//     }

//     #[derive(Subcommand)]
//     enum Commands {
//         GetDirectories {
//             #[arg(short, long)]
//             dir: String,
//         },
//         GetAllFiles {
//             #[arg(short, long)]
//             dir: String,
//         },
//     }

//     let cli = Cli::parse();

//     let addr = match cli.address {
//         Some(addr) => addr,
//         None => "http://127.0.0.1:9001".to_string(),
//     };
//     let destination = match cli.destination {
//         Some(destination) => destination,
//         None => "testdestination".to_string(),
//     };

//     let c = Arc::new(Client::new(destination));

//     match cli.command {
//         Some(Commands::GetDirectories { dir }) => {
//             println!("{:?}", c.get_directory(dir, addr.clone()).await)
//         }
//         Some(Commands::GetAllFiles { dir }) => {
//             let files = c.get_directory(dir, addr.clone()).await.unwrap();
//             c.get_all_files(addr, files).await.unwrap();
//         }
//         _ => {
//             return Ok(());
//         }
//     };

//     Ok(())
// }
