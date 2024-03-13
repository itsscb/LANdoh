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

// // use self::source::Source;

mod multicast {
    include!("multicast.rs");
}

use self::multicast::{
    receiver::{self, Source},
    Sender,
};

pub use self::server::{Directory, Server};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Payload<T> {
    id: String,
    data: PayloadEnum<T>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PayloadEnum<T> {
    One(T),
    Many(T),
}

pub type Sources = Arc<Mutex<Vec<Source>>>;

pub struct App {
    pub config: Config,
    handles: JoinSet<()>,
    sender: Sender,
    sources: Sources,
}

impl App {
    pub fn new(config: Config) -> Self {
        App {
            config: config,
            handles: JoinSet::new(),
            sender: Sender::new().unwrap(),
            sources: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn listen(&mut self) {
        let a = Arc::clone(&self.sources);
        let id = self.config.id.to_string();
        self.handles.spawn(async move {
            receiver::listen(id, a).unwrap();
        });
    }

    pub fn publish(&self, payload: Source) -> Result<(), Box<dyn Error>> {
        self.sender.send(payload)
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

        let dir = Directory {
            name: name.to_string(),
            paths: existing_paths,
        };
        let _ = &self
            .config
            .shared_directories
            .lock()
            .unwrap()
            .push(dir.clone());

        self.publish(Source::new(
            self.config.id.to_string(),
            self.config.nickname.clone(),
            None,
            self.config
                .shared_directories
                .lock()
                .unwrap()
                .iter()
                .map(|i| i.name.clone())
                .collect(),
        ))?;

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
