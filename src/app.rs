use std::{
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use uuid::Uuid;

mod server {
    include!("server.rs");
}

pub use self::server::{Directory, Server};

pub struct App {
    // pub server: Server,
    pub config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        App { config: config }
    }

    pub async fn serve(&self) -> Result<(), Box<dyn Error>> {
        let a = Arc::clone(&self.config.shared_directories);
        let server = Server::new_with_data(a);
        server.serve(self.config.address).await
    }
}

pub struct Config {
    pub id: Uuid,
    pub nickname: String,
    pub shared_directories: Arc<Mutex<Vec<Directory>>>,
    pub destination: PathBuf,
    pub address: SocketAddr,
}

impl Config {
    pub fn new(
        shared_directories: Vec<String>,
        destination: String,
        address: SocketAddr,
        nickname: Option<String>,
    ) -> Result<Self, Box<dyn Error>> {
        let id = Uuid::new_v4();
        let dirs: Arc<Mutex<Vec<Directory>>> = Arc::new(Mutex::new(
            shared_directories
                .iter()
                .map(|d| Directory {
                    name: d.to_string(),
                    paths: vec![d.to_string()],
                })
                .collect(),
        ));
        let dest = PathBuf::from(destination);

        let nick = match nickname {
            Some(n) => n,
            None => id.to_string(),
        };

        Ok(Config {
            id: id,
            shared_directories: dirs,
            nickname: nick,
            destination: dest,
            address: address,
        })
    }
}
