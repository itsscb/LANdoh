use std::{
    env,
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};
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

fn save_config(config: &Config) -> Result<(), Box<dyn Error>> {
    let path = config_path();

    let mut f = OpenOptions::new();

    fs::create_dir_all(path.parent().unwrap())?;

    let mut file = f.write(true).create(true).open(&path)?;
    let payload = toml::to_string_pretty(&config)?;
    file.write_all(payload.as_bytes())?;

    Ok(())
}

#[cfg(windows)]
fn config_path() -> PathBuf {
    let mut appdata = env::var("APPDATA").unwrap();

    appdata.extend(["landoh", "config.ini"]);
    PathBuf::from(appdata)
}

#[cfg(unix)]
fn config_path() -> PathBuf {
    let mut appdata = env::var("HOME").unwrap();

    appdata.extend(["/", ".landoh_config"]);

    PathBuf::from(appdata)
}

pub struct App {
    pub config: Config,
    pub handles: JoinSet<()>,
    sender: Arc<Mutex<Sender>>,
    pub sources: Sources,
}

impl App {
    pub fn new(config: Config) -> Self {
        save_config(&config).unwrap();
        App {
            config: config,
            handles: JoinSet::new(),
            sender: Arc::new(Mutex::new(Sender::new().unwrap())),
            sources: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn new_from_config() -> Result<Self, Box<dyn Error>> {
        let path = config_path();
        match path.exists() {
            true => match path.is_file() {
                true => {
                    let mut f = File::open(path)?;
                    let mut s: String = "".to_string();
                    let _ = f.read_to_string(&mut s)?;
                    let c: Config = toml::from_str::<Config>(&s)?;
                    return Ok(Self::new(c));
                }
                false => {
                    return Err("Config not found".into());
                }
            },
            false => {
                return Err("Config file not found".into());
            }
        };
    }

    pub fn listen(&mut self) {
        let a = Arc::clone(&self.sources);
        let id = self.config.id.to_string();
        self.handles.spawn(async move {
            receiver::listen(id, a).unwrap();
        });
    }

    pub fn broadcast(&mut self) {
        let id = self.config.id.clone();
        let nickname = self.config.nickname.clone();
        let s = Arc::clone(&self.sender);
        let dirs = Arc::clone(&self.config.shared_directories);
        self.handles.spawn(async move {
            loop {
                thread::sleep(Duration::from_secs(30));
                let _ = s.lock().unwrap().send(Source::new(
                    id.to_string(),
                    nickname.to_string(),
                    None,
                    dirs.lock()
                        .unwrap()
                        .iter()
                        .map(|d| d.name.clone())
                        .collect(),
                ));
            }
        });
    }

    pub fn publish(&self, payload: Source) -> Result<(), Box<dyn Error>> {
        self.sender.lock().unwrap().send(payload)
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

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub id: String,
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
        let id = Uuid::new_v4().to_string();
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
