use std::{
    error::Error,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use uuid::Uuid;

use tokio::task::JoinSet;

mod server {
    include!("server.rs");
}

pub use self::server::{Directory, Server};

pub struct App {
    pub config: Config,
    pub handles: JoinSet<()>,
}

impl App {
    pub fn new(config: Config) -> Self {
        App {
            config: config,
            handles: JoinSet::new(),
        }
    }

    pub async fn serve(&mut self) {
        let a = Arc::clone(&self.config.shared_directories);
        let server = Server::new(a);
        let addr = self.config.address;
        self.handles.spawn(async move {
            let _ = server.serve(addr).await;
        });
    }

    pub async fn join_all(&mut self) {
        while let Some(_) = self.handles.join_next().await {}
    }

    #[allow(dead_code)]
    pub fn add_shared_dir(
        &mut self,
        name: String,
        paths: Vec<String>,
    ) -> Result<(), Box<dyn Error>> {
        let mut existing_paths = vec![];

        for pa in paths {
            let p = PathBuf::from(&pa);
            if p.exists() && p.metadata()?.is_dir() {
                existing_paths.push(pa);
            }
        }
        let _ = &self
            .config
            .shared_directories
            .lock()
            .unwrap()
            .push(Directory {
                name: name.to_string(),
                paths: existing_paths,
            });
        Ok(())
    }

    #[allow(dead_code)]
    pub fn remove_shared_dir(&mut self, name: String) {
        self.config
            .shared_directories
            .lock()
            .unwrap()
            .retain(|d| d.name != name);
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
