use std::{
    env,
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::Duration,
};

use tokio::sync::Mutex;

use log::info;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use tokio::task::JoinSet;

use crate::multicast::{
    receiver::{self, Source},
    Sender,
};

pub use crate::server::{Directory, Server};

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

    fs::remove_file(&path).unwrap_or_default();

    let mut file = f.write(true).create(true).open(&path)?;

    let payload = serde_json::to_string_pretty(&config)?;
    file.write_all(payload.as_bytes())?;

    Ok(())
}

#[cfg(windows)]
fn config_path() -> PathBuf {
    let mut appdata = env::var("APPDATA").unwrap();

    appdata.extend(["/", "LANdoh", "/", "config.ini"]);
    PathBuf::from(appdata)
}

#[cfg(unix)]
fn config_path() -> PathBuf {
    let mut appdata = env::var("HOME").unwrap();

    appdata.extend(["/", ".landoh_config"]);

    PathBuf::from(appdata)
}

#[allow(dead_code)]
#[derive(Debug)]
enum LogLevel {
    INFO,
    DEBUG,
    WARN,
    ERROR,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn set_loglevel(lvl: LogLevel) {
    env::set_var("RUST_LOG", lvl.to_string());
}

#[derive(Debug)]
pub struct App {
    pub config: Arc<Mutex<Config>>,
    pub handles: JoinSet<()>,
    sender: Arc<Mutex<Sender>>,
    pub sources: Sources,
}

impl App {
    pub fn new(config: Config) -> Self {
        set_loglevel(LogLevel::INFO);
        let _ = env_logger::try_init();

        save_config(&config).unwrap();

        App {
            config: Arc::new(Mutex::new(config)),
            handles: JoinSet::new(),
            sender: Arc::new(Mutex::new(Sender::new().unwrap())),
            sources: Arc::new(Mutex::new(vec![])),
        }
    }

    pub fn new_from_config() -> Result<Self, Box<dyn Error>> {
        set_loglevel(LogLevel::INFO);

        let _ = env_logger::try_init();

        let path = config_path();
        match path.exists() {
            true => match path.is_file() {
                true => {
                    let mut f = File::open(path)?;
                    let mut s: String = "".to_string();
                    let _ = f.read_to_string(&mut s)?;
                    let c: Config = serde_json::from_str::<Config>(&s)?;
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

    pub async fn save_config(&self) {
        save_config(&self.config.lock().await.clone()).unwrap();
    }

    pub async fn listen(&mut self) -> Receiver<Vec<Source>> {
        let s = Arc::clone(&self.sources);
        let id = self.config.lock().await.id.to_string();
        let (tx, rx) = mpsc::channel::<Vec<Source>>();
        self.handles.spawn(async move {
            let _ = receiver::listen(id, s, Some(tx)).await;
        });
        rx
    }

    pub async fn broadcast(&mut self) {
        let s = Arc::clone(&self.sender);
        let dirs = Arc::clone(&self.config);
        tokio::spawn(async move {
            loop {
                {
                    let c = dirs.lock().await;
                    let d = Source::new(
                        c.id.to_string().clone(),
                        c.nickname.to_string().clone(),
                        None,
                        c.shared_directories
                            .clone()
                            .iter()
                            .map(|d| d.name.clone())
                            .collect(),
                    );
                    let _ = s.lock().await.send(d).await;
                }
                thread::sleep(Duration::from_secs(5));
            }
        });
    }

    pub async fn publish(&self, payload: Source) -> Result<(), Box<dyn Error>> {
        self.sender.lock().await.send(payload).await
    }

    pub async fn serve(&mut self) {
        let s = self.config.lock().await;
        let c = Arc::clone(&self.config);
        let server = Server::new(c);
        let addr = s.address;
        let _ = env_logger::try_init();

        info!(
            "serving backend on {} as {}\nID: {}",
            &addr.ip().to_string(),
            &s.nickname,
            &s.id,
        );
        self.handles.spawn(async move {
            let _ = server.serve(addr.clone()).await;
        });
    }

    pub async fn join_all(&mut self) {
        while let Some(_) = self.handles.join_next().await {}
    }

    pub async fn add_shared_dir(
        &mut self,
        name: String,
        paths: Vec<String>,
    ) -> Result<(), Box<dyn Error>> {
        let mut na = name;
        let mut existing_paths = vec![];

        if self
            .config
            .lock()
            .await
            .shared_directories
            .iter()
            .any(|d| d.name == na)
        {
            return Ok(());
        }

        for pa in paths {
            let p = PathBuf::from(&pa);
            if p.exists() && p.metadata()?.is_dir() {
                existing_paths.push(pa);
            }
        }

        let p = PathBuf::from(&na);
        if p.exists() && p.is_dir() {
            na = match p.file_name() {
                Some(n) => n.to_str().unwrap().to_string(),
                None => na,
            }
        }

        let dir = Directory {
            name: na,
            paths: existing_paths,
        };

        {
            let _ = &self
                .config
                .lock()
                .await
                .shared_directories
                // .lock()
                // .await
                .push(dir.clone());
        }
        {
            let c = self.config.lock().await;

            self.publish(Source::new(
                c.id.to_string(),
                c.nickname.clone(),
                None,
                c.shared_directories
                    // .lock()
                    // .await
                    .iter()
                    .map(|i| i.name.clone())
                    .collect(),
            ))
            .await?;
        }
        Ok(())
    }

    pub async fn remove_shared_dir(&mut self, name: String) {
        self.config
            .lock()
            .await
            .shared_directories
            // .lock()
            // .await
            .retain(|d| d.name != name);
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Config {
    pub id: String,
    pub nickname: String,
    pub shared_directories: Vec<Directory>,
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
        let dirs = shared_directories
            .iter()
            .map(|d| Directory {
                name: d.to_string(),
                paths: vec![d.to_string()],
            })
            .collect();

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
