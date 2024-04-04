use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot, RwLock,
};

use uuid::Uuid;

#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};

use super::proto::pb::{self, Directory};
use super::{Order, OrderResponse};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Server {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared: Arc<RwLock<Vec<Directory>>>,
    event_listening: Arc<RwLock<bool>>,
    broadcasting: Arc<RwLock<bool>>,
    serving: Arc<RwLock<bool>>,
    tx: Option<Sender<Order>>,
    serve_addr: SocketAddr,
}

impl Server {
    pub fn builder() -> Builder {
        Builder::new()
    }

    pub async fn is_broadcasting(&self) -> bool {
        *self.broadcasting.read().await
    }
    pub async fn is_event_listening(&self) -> bool {
        *self.event_listening.read().await
    }
    pub fn get_id(&self) -> String {
        self.id.clone()
    }
    pub fn get_address(&self) -> String {
        self.serve_addr.to_string()
    }
    pub async fn get_nickname(&self) -> String {
        self.nickname.read().await.clone().to_string()
    }

    pub async fn send(&self, m: Order) {
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
                *serving.write().await = true;
            }
            let _ = tonic::transport::Server::builder()
                .add_service(pb::lan_doh_server::LanDohServer::new(self))
                .add_service(reflection_service)
                .serve_with_shutdown(addr, async { drop(rx.await) })
                .await;

            {
                *serving.write().await = false;
            }
        });
        tx
    }

    #[allow(dead_code)]
    pub async fn run(self) -> (Sender<Order>, Receiver<OrderResponse>) {
        let mut sv = self.clone();

        let (reply_tx, reply_rx) = mpsc::channel::<OrderResponse>(128);
        let (tx, rx) = mpsc::channel::<Order>(128);
        sv.tx = Some(tx.clone());
        info!(
            "Starting up LANdoh backend on {} as {} with ID: {}",
            sv.serve_addr.to_string(),
            *sv.nickname.read().await,
            sv.id
        );
        sv.send(Order::StartServe(None)).await;

        if *sv.event_listening.read().await {
            sv.send(Order::StartListen).await;
            info!("EventListener: Enabled");
        } else {
            info!("EventListener: Disabled");
        }
        if *sv.broadcasting.read().await {
            sv.send(Order::StartBroadcast).await;
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
                        Order::GotRequest(r) => {
                            debug!("{}", r);
                            let _ = rtx.send(OrderResponse::Done(Order::GotRequest(r))).await;
                        }
                        Order::SendFile(p) => {
                            debug!("Got 'SendFile' Order: {}", &p);
                            let _ = rtx.send(OrderResponse::Done(Order::SendFile(p))).await;
                        }
                        Order::AddDir(d) => {
                            debug!("Got 'AddDir' Request: {:?}", &d);
                            let dir = d.to_string_lossy().to_string();
                            {
                                sv.shared.write().await.push(Directory {
                                    name: dir.clone(),
                                    paths: vec![dir],
                                });
                            }
                            let _ = rtx.send(OrderResponse::Done(Order::AddDir(d))).await;
                        }
                        Order::RemoveDir(name) => {
                            debug!("Got 'RemoveDir' Request: {:?}", &name);
                            {
                                sv.shared
                                    .write()
                                    .await
                                    .retain(|d| d.name != name.to_string_lossy().to_string());
                            }
                            let _ = rtx.send(OrderResponse::Done(Order::RemoveDir(name))).await;
                        }
                        Order::SetNickname(n) => {
                            debug!("Got 'SetNickname' Request: {}", &n);
                            {
                                *sv.nickname.write().await = n.clone();
                            }
                            let _ = rtx.send(OrderResponse::Done(Order::SetNickname(n))).await;
                        }
                        Order::StartBroadcast => {
                            debug!("Got 'StartBroadcast' Request");
                            let _ = rtx.send(OrderResponse::Done(Order::StartBroadcast)).await;
                        }
                        Order::StopBroadcast => {
                            debug!("Got 'StopBroadcast' Request");
                            let _ = rtx.send(OrderResponse::Done(Order::StartBroadcast)).await;
                        }
                        Order::StartListen => {
                            debug!("Got 'StartListen' Request");
                            let _ = rtx.send(OrderResponse::Done(Order::StartListen)).await;
                        }
                        Order::StopListen => {
                            debug!("Got 'StopListen' Request");
                            let _ = rtx.send(OrderResponse::Done(Order::StopListen)).await;
                        }
                        Order::StartServe(addr) => {
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
                                    .send(OrderResponse::Done(Order::StartServe(addr)))
                                    .await;
                            } else {
                                let _ = rtx
                                    .clone()
                                    .send(OrderResponse::Err(format!(
                                        "Already serving on: {}",
                                        sv.serve_addr.to_string()
                                    )))
                                    .await;
                            }
                        }
                        Order::StopServe => {
                            debug!("Got 'StopServe' Request");
                            if let Some(t) = serve_exit_tx.take() {
                                match t.send(()) {
                                    Ok(_) => {
                                        let _ =
                                            rtx.send(OrderResponse::Done(Order::StopServe)).await;
                                    }
                                    Err(_) => {
                                        let _ = rtx
                                            .send(OrderResponse::Err(
                                                "failed to stop serving".to_string(),
                                            ))
                                            .await;
                                    }
                                };
                            }
                        }
                        Order::Exit => {
                            debug!("SENDING EXIT REQUESTS...");
                            let _ = tx.send(Order::StopListen).await;
                            debug!("SEND EXIT REQUEST: StopListen");
                            let _ = tx.send(Order::StopBroadcast).await;
                            debug!("SEND EXIT REQUEST: StopBroadcast");
                            let _ = tx.send(Order::StopServe).await;
                            debug!("SEND EXIT REQUEST: StopServe");
                            let _ = rtx.send(OrderResponse::Done(Order::Exit)).await;
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Builder {
    id: String,
    nickname: String,
    shared: Vec<PathBuf>,
    listen: bool,
    broadcasting: bool,
    serve_addr: SocketAddr,
}

impl Builder {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let id = Uuid::new_v4().to_string();
        Self {
            id: id.clone(),
            nickname: id,
            shared: Vec::new(),
            listen: false,
            broadcasting: false,
            serve_addr: SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), 50051),
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
fn test_builder() {
    let nick = "my_nick".to_string();
    let path_1 = PathBuf::from("C:\\temp");
    let path_2 = PathBuf::from("/home/user01/shared");
    let sb = Builder::new()
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
