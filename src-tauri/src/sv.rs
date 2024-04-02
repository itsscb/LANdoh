use core::fmt;
use std::{env, path::PathBuf, pin::Pin};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, RwLock},
};

use serde::Deserialize;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use uuid::Uuid;

#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};

use self::pb::get_file_response::FileResponse;
use self::pb::{
    lan_doh_server::LanDoh, Directory, GetDirectoryRequest, GetDirectoryResponse, GetFileRequest,
    GetFileResponse, ListDirectoriesRequest, ListDirectoriesResponse,
};
use self::pb::{FileMetaData, HealthzRequest, HealthzResponse};

mod pb {
    tonic::include_proto!("pb");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("pb_descriptor");
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Message {
    GotRequest(Request),
    AddDir(PathBuf),
    SendFile(String),
    RemoveDir(PathBuf),
    SetNickname(String),
    StopBroadcast,
    StartBroadcast,
    StopServe,
    StartServe(Option<SocketAddr>),
    StartListen,
    StopListen,
    Exit,
}

#[derive(Debug)]
pub struct Request {
    pub remote_address: Option<SocketAddr>,
    pub service: Service,
}

impl Request {
    pub fn new<T>(service: Service, request: tonic::Request<T>) -> Self {
        Self {
            remote_address: request.remote_addr(),
            service: service,
        }
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.remote_address {
            Some(addr) => {
                write!(
                    f,
                    "{} requested {}",
                    addr.to_string(),
                    self.service.to_string()
                )
            }
            None => {
                write!(f, "UNKNOWN requested {}", self.service.to_string())
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Service {
    Healthz,
    GetDirectory,
    GetFile,
    ListDirectories,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub enum MessageResponse {
    Done(Message),
    Err(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Server {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared: Arc<RwLock<Vec<Directory>>>,
    event_listening: Arc<RwLock<bool>>,
    broadcasting: Arc<RwLock<bool>>,
    serving: Arc<RwLock<bool>>,
    tx: Option<Sender<Message>>,
    serve_addr: SocketAddr,
}

impl Server {
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    pub async fn send(&self, m: Message) {
        if let Some(t) = &self.tx {
            let _ = t.send(m).await;
        }
    }

    pub fn serve(self) -> oneshot::Sender<()> {
        let (tx, rx) = oneshot::channel::<()>();
        let serving = self.serving.clone();
        tokio::spawn(async move {
            let reflection_service = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(pb::FILE_DESCRIPTOR_SET)
                .build()
                .unwrap();

            let addr = self.serve_addr;

            {
                *serving.write().unwrap() = true;
            }
            let _ = tonic::transport::Server::builder()
                .add_service(pb::lan_doh_server::LanDohServer::new(self))
                .add_service(reflection_service)
                .serve_with_shutdown(addr, async { drop(rx.await) })
                .await;

            {
                *serving.write().unwrap() = false;
            }
        });
        tx
    }

    #[allow(dead_code)]
    pub async fn run(self) -> (Sender<Message>, Receiver<MessageResponse>) {
        let mut sv = self.clone();

        let (reply_tx, reply_rx) = mpsc::channel::<MessageResponse>(128);
        let (tx, rx) = mpsc::channel::<Message>(128);
        sv.tx = Some(tx.clone());
        info!(
            "Starting up LANdoh backend on {} as {} with ID: {}",
            sv.serve_addr.to_string(),
            *sv.nickname.read().unwrap(),
            sv.id
        );

        if *sv.event_listening.read().unwrap() {
            sv.send(Message::StartListen).await;
            info!("EventListener: Enabled");
        } else {
            info!("EventListener: Disabled");
        }
        if *sv.broadcasting.read().unwrap() {
            sv.send(Message::StartBroadcast).await;
            info!("Broadcasting: Enabled");
        } else {
            info!("Broadcasting: Disabled");
        }

        let atx = tx.clone();
        let _ = tokio::spawn(async move {
            let mut serve_exit_tx: Option<oneshot::Sender<()>> = None;
            let mut rx = rx;
            let tx = atx.clone();
            let rtx = reply_tx;

            loop {
                match rx.recv().await {
                    Some(msg) => match msg {
                        Message::GotRequest(r) => {
                            debug!("{}", r);
                            let _ = rtx
                                .send(MessageResponse::Done(Message::GotRequest(r)))
                                .await;
                        }
                        Message::SendFile(p) => {
                            debug!("Got 'SendFile' Message: {}", &p);
                            let _ = rtx.send(MessageResponse::Done(Message::SendFile(p))).await;
                        }
                        Message::AddDir(d) => {
                            debug!("Got 'AddDir' Request: {:?}", &d);
                            let dir = d.to_string_lossy().to_string();
                            {
                                sv.shared.write().unwrap().push(Directory {
                                    name: dir.clone(),
                                    paths: vec![dir],
                                });
                            }
                            let _ = rtx.send(MessageResponse::Done(Message::AddDir(d))).await;
                        }
                        Message::RemoveDir(name) => {
                            debug!("Got 'RemoveDir' Request: {:?}", &name);
                            {
                                sv.shared
                                    .write()
                                    .unwrap()
                                    .retain(|d| d.name != name.to_string_lossy().to_string());
                            }
                            let _ = rtx
                                .send(MessageResponse::Done(Message::RemoveDir(name)))
                                .await;
                        }
                        Message::SetNickname(n) => {
                            debug!("Got 'SetNickname' Request: {}", &n);
                            {
                                *sv.nickname.write().unwrap() = n.clone();
                            }
                            let _ = rtx
                                .send(MessageResponse::Done(Message::SetNickname(n)))
                                .await;
                        }
                        Message::StartBroadcast => {
                            debug!("Got 'StartBroadcast' Request");
                            let _ = rtx
                                .send(MessageResponse::Done(Message::StartBroadcast))
                                .await;
                        }
                        Message::StopBroadcast => {
                            debug!("Got 'StopBroadcast' Request");
                            let _ = rtx
                                .send(MessageResponse::Done(Message::StartBroadcast))
                                .await;
                        }
                        Message::StartListen => {
                            debug!("Got 'StartListen' Request");
                            let _ = rtx.send(MessageResponse::Done(Message::StartListen)).await;
                        }
                        Message::StopListen => {
                            debug!("Got 'StopListen' Request");
                            let _ = rtx.send(MessageResponse::Done(Message::StopListen)).await;
                        }
                        Message::StartServe(addr) => {
                            debug!("Got 'StartServe' Request: {:?}", &addr);

                            let mut start = false;

                            if let Some(ref t) = serve_exit_tx {
                                if t.is_closed() {
                                    start = true;
                                }
                            }

                            if let None = serve_exit_tx {
                                start = true;
                            }

                            if start {
                                match addr {
                                    Some(a) => sv.serve_addr = a,
                                    None => {}
                                };
                                serve_exit_tx = Some(sv.clone().serve());
                                let _ = rtx
                                    .clone()
                                    .send(MessageResponse::Done(Message::StartServe(addr)))
                                    .await;
                            } else {
                                let _ = rtx
                                    .clone()
                                    .send(MessageResponse::Err(format!(
                                        "Already serving on: {}",
                                        sv.serve_addr.to_string()
                                    )))
                                    .await;
                            }
                        }
                        Message::StopServe => {
                            debug!("Got 'StopServe' Request");
                            if let Some(t) = serve_exit_tx.take() {
                                match t.send(()) {
                                    Ok(_) => {
                                        let _ = rtx
                                            .send(MessageResponse::Done(Message::StopServe))
                                            .await;
                                    }
                                    Err(_) => {
                                        let _ = rtx
                                            .send(MessageResponse::Err(
                                                "failed to stop serving".to_string(),
                                            ))
                                            .await;
                                    }
                                };
                            }
                        }
                        Message::Exit => {
                            debug!("SENDING EXIT REQUESTS...");
                            let _ = tx.send(Message::StopListen).await;
                            debug!("SEND EXIT REQUEST: StopListen");
                            let _ = tx.send(Message::StopBroadcast).await;
                            debug!("SEND EXIT REQUEST: StopBroadcast");
                            let _ = tx.send(Message::StopServe).await;
                            debug!("SEND EXIT REQUEST: StopServe");
                            let _ = rtx.send(MessageResponse::Done(Message::Exit)).await;
                            break;
                        }
                    },
                    None => {}
                }
            }
        });
        (tx, reply_rx)
    }
}

#[tonic::async_trait]
impl LanDoh for Server {
    type GetFileStream = Pin<Box<dyn Stream<Item = Result<GetFileResponse, tonic::Status>> + Send>>;

    async fn healthz(
        &self,
        request: tonic::Request<HealthzRequest>,
    ) -> Result<tonic::Response<HealthzResponse>, tonic::Status> {
        self.send(Message::GotRequest(Request::new(Service::Healthz, request)))
            .await;

        Ok(tonic::Response::new(HealthzResponse {
            broadcaster: *self.broadcasting.read().unwrap(),
            event_listener: *self.event_listening.read().unwrap(),
            address: self.serve_addr.to_string(),
            id: self.id.clone(),
            nickname: self.nickname.read().unwrap().to_string(),
        }))
    }

    async fn list_directories(
        &self,
        _request: tonic::Request<ListDirectoriesRequest>,
    ) -> Result<tonic::Response<ListDirectoriesResponse>, tonic::Status> {
        unimplemented!("SV::LIST_DIRECTORIES");
    }
    async fn get_directory(
        &self,
        _request: tonic::Request<GetDirectoryRequest>,
    ) -> Result<tonic::Response<GetDirectoryResponse>, tonic::Status> {
        unimplemented!("SV::GET_DIRECTORY");
    }

    async fn get_file(
        &self,
        request: tonic::Request<GetFileRequest>,
    ) -> Result<tonic::Response<Self::GetFileStream>, tonic::Status> {
        let (tx, rx): (
            Sender<Result<GetFileResponse, tonic::Status>>,
            Receiver<Result<GetFileResponse, tonic::Status>>,
        ) = mpsc::channel(128);

        info!("Got 'GetFile' Request: {:?}", request);

        tokio::spawn(async move {
            let path = "pseudo_path".to_string();
            let hash = "psuedo_hash".to_string();

            loop {
                let _ = tx
                    .send(Ok(GetFileResponse {
                        file_response: Some(FileResponse::Meta(FileMetaData {
                            file_size: 0,
                            path: path.clone(),
                            hash: hash.clone(),
                        })),
                    }))
                    .await;
            }
        });

        let output_stream: ReceiverStream<Result<GetFileResponse, tonic::Status>> =
            ReceiverStream::new(rx);

        Ok(tonic::Response::new(
            Box::pin(output_stream) as Self::GetFileStream
        ))
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ServerBuilder {
    id: String,
    nickname: String,
    shared: Vec<PathBuf>,
    listen: bool,
    broadcasting: bool,
    serve_addr: SocketAddr,
}

impl ServerBuilder {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id: id.clone(),
            nickname: id,
            shared: Vec::new(),
            listen: false,
            broadcasting: false,
            serve_addr: SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), 9001),
        }
    }

    pub fn build(self) -> Server {
        Server {
            id: self.id,
            nickname: Arc::new(RwLock::new(self.nickname)),
            shared: Arc::new(RwLock::new(
                self.shared
                    .iter()
                    .map(|d| Directory {
                        name: d.to_string_lossy().to_string(),
                        paths: vec![d.to_string_lossy().to_string()],
                    })
                    .collect(),
            )),
            event_listening: Arc::new(RwLock::new(self.listen)),
            broadcasting: Arc::new(RwLock::new(self.broadcasting)),
            serving: Arc::new(RwLock::new(false)),
            tx: None,
            serve_addr: self.serve_addr,
        }
    }

    #[allow(dead_code)]
    pub fn set_serve_address(mut self, addr: SocketAddr) -> Self {
        self.serve_addr = addr;
        self
    }

    #[allow(dead_code)]
    pub fn add_share(mut self, path: PathBuf) -> Self {
        if self.shared.iter().any(|i| *i == path) {
            return self;
        }
        self.shared.push(path);
        self
    }

    #[allow(dead_code)]
    pub fn set_nickname(mut self, name: String) -> Self {
        self.nickname = name;
        self
    }

    #[allow(dead_code)]
    pub fn enable_eventlistening(mut self) -> Self {
        self.listen = true;
        self
    }

    #[allow(dead_code)]
    pub fn enable_broadcasting(mut self) -> Self {
        self.broadcasting = true;
        self
    }
}

#[test]
fn test_serverbuilder() {
    let nick = "my_nick".to_string();
    let path_1 = PathBuf::from("C:\\temp");
    let path_2 = PathBuf::from("/home/user01/shared");
    let sb = ServerBuilder::new()
        .add_share(path_1.clone())
        .add_share(path_2.clone())
        .enable_eventlistening()
        .enable_broadcasting()
        .set_nickname(nick.clone());

    assert_eq!(sb.nickname, nick);
    assert_eq!(sb.shared[0], path_1);
    assert_eq!(sb.shared[1], path_2);
    assert!(sb.listen);
    assert!(sb.broadcasting);

    assert_ne!(sb.id, "".to_string());
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SvBuilder {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared: Arc<RwLock<Vec<Directory>>>,
    manager_tx: Sender<Message>,
    manager_rx: Receiver<Message>,
}

impl SvBuilder {
    #[allow(dead_code)]
    pub fn new(nickname: String, shared: Vec<String>) -> Self {
        let (tx, rx) = mpsc::channel::<Message>(128);
        let nick = Arc::new(RwLock::new(nickname));
        let dirs = Arc::new(RwLock::new(
            shared
                .iter()
                .map(|d| Directory {
                    name: d.to_string(),
                    paths: vec![d.to_string()],
                })
                .collect(),
        ));

        Self {
            id: Uuid::new_v4().to_string(),
            nickname: nick,
            manager_tx: tx,
            manager_rx: rx,
            shared: dirs,
        }
    }
}

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "DEBUG".to_string());
    let _ = env_logger::init();

    let s = Server::builder()
        .set_nickname("MyNick".to_string())
        .add_share(PathBuf::from("C:\\temp"))
        .enable_eventlistening()
        .enable_broadcasting()
        .build();

    let (t, mut r) = s.run().await;
    let _ = t.send(Message::AddDir(PathBuf::from("C:/Test1"))).await;
    let _ = t.send(Message::RemoveDir(PathBuf::from("Test1"))).await;
    let _ = t.send(Message::SetNickname("Nick_123".to_string())).await;
    let _ = t
        .send(Message::StartServe(Some(SocketAddr::new(
            Ipv4Addr::new(0, 0, 0, 0).into(),
            50051,
        ))))
        .await;
    let _ = t.send(Message::StartBroadcast).await;
    let _ = t.send(Message::StartListen).await;

    // thread::sleep(Duration::from_secs(20));

    // let _ = t.send(Message::Exit).await;

    while let Some(msg) = r.recv().await {
        match msg {
            MessageResponse::Done(m) => {
                info!("DONE = {:?}", m);
            }
            MessageResponse::Err(e) => {
                info!("ERROR = {:?}", e);
            }
        };
    }
    Ok(())
}
