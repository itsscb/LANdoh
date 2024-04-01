use std::thread;
use std::time::Duration;
use std::{env, path::PathBuf, pin::Pin};
use std::{net::{Ipv4Addr, SocketAddr}, sync::{Arc, RwLock}};

use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot::{self, Sender as oSender};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tonic::{async_trait, Request as tRequest, Response as tResponse, Status, transport::Server as tServer};
use uuid::Uuid;


#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};

use self::pb::get_file_response::FileResponse;
use self::pb::FileMetaData;
use self::pb::{Directory, lan_doh_server::LanDoh, GetDirectoryRequest, GetDirectoryResponse, GetFileRequest, GetFileResponse, ListDirectoriesRequest, ListDirectoriesResponse};

mod pb {
    tonic::include_proto!("pb");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("pb_descriptor");
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Message {
    AddDir(PathBuf),
    SendFile(String),
    RemoveDir(PathBuf),
    SetNickname(String),
    StopBroadcast,
    StartBroadcast,
    StopServe,
    StartServe(SocketAddr),
    StartListen,
    StopListen,
    Exit
}

#[derive(Debug)]
pub enum Response {
    Done(Message),
    Err(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Sv {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared: Arc<RwLock<Vec<Directory>>>,
    tx: Sender<Message>,
}

impl Sv {
    #[allow(dead_code)]
    pub async fn send(&self, m: Message) -> Result<(),tokio::sync::mpsc::error::SendError<Message>> {
        self.tx.send(m).await
    }
}

#[async_trait]
impl LanDoh for Sv {
    type GetFileStream = Pin<Box<dyn Stream<Item = Result<GetFileResponse, Status>> + Send>>;
    async fn list_directories(
        &self,
        _request: tRequest<ListDirectoriesRequest>,
    ) -> Result<tResponse<ListDirectoriesResponse>, Status> {
        unimplemented!("SV::LIST_DIRECTORIES");
        // Ok(tResponse::new(ListDirectoriesResponse {
        //     dirs: vec![] }))
        }
    async fn get_directory(
        &self,
        _request: tRequest<GetDirectoryRequest>,
    ) -> Result<tResponse<GetDirectoryResponse>, Status> {
        // Ok(tResponse::new(GetDirectoryResponse{files: vec![]}))
        unimplemented!("SV::GET_DIRECTORY");
    }

    async fn get_file(
        &self,
        request: tRequest<GetFileRequest>,
    ) -> Result<tResponse<Self::GetFileStream>, Status> {
        let (tx, rx): (
            Sender<Result<GetFileResponse, Status>>,
            Receiver<Result<GetFileResponse, Status>>,
        ) = mpsc::channel(128);

        info!("Got 'GetFile' Request: {:?}", request);

        // let txx = self.tx.clone();
        tokio::spawn(async move {
            let path = "pseudo_path".to_string();
            let hash = "psuedo_hash".to_string();

            loop {
                // let _ = txx.send(Message::SendFile(path.clone())).await;
                let _ = tx.send(Ok(GetFileResponse {
                    file_response: Some(FileResponse::Meta(FileMetaData{
                        file_size: 0,
                        path: path.clone(),
                        hash: hash.clone(),
                    }))
                })).await;
            }
        });

        let output_stream: ReceiverStream<Result<GetFileResponse, Status>> =
            ReceiverStream::new(rx);

        Ok(tResponse::new(Box::pin(output_stream) as Self::GetFileStream))
    }
}


#[allow(dead_code)]
    pub fn serve(sv: Sv, addr: SocketAddr) -> oSender<()> {
        let (tx, rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(pb::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();
        
            let _ = tServer::builder()
            .add_service(pb::lan_doh_server::LanDohServer::new(sv))
            .add_service(reflection_service)
            .serve_with_shutdown(addr, async { drop(rx.await)} ).await;        
    });
        tx
    }

#[allow(dead_code)]
#[derive(Debug)]
pub struct SvBuilder {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared: Arc<RwLock<Vec<Directory>>>,
    manager_tx: Sender<Message>,
    manager_rx: Receiver<Message>
}

impl SvBuilder {
    #[allow(dead_code)]
    pub fn new(nickname: String, shared: Vec<String>) -> Self {
        let (tx, rx) = mpsc::channel::<Message>(128);
        let nick = Arc::new(RwLock::new(nickname));
        let dirs = Arc::new(RwLock::new(shared
            .iter()
            .map(|d| Directory {
                name: d.to_string(),
                paths: vec![d.to_string()],
            })
            .collect()));

        Self{
            id:Uuid::new_v4().to_string(),
            nickname: nick,
            manager_tx: tx,
            manager_rx: rx,
            shared: dirs,
        }      
}


#[allow(dead_code)]
pub async fn run(self) -> (Sender<Message>, Receiver<Response>){
    let (reply_tx, reply_rx) = mpsc::channel::<Response>(128);

    let nickname = self.nickname;
    let shared = self.shared;
    let id = self.id;
    let rx = self.manager_rx;
    let tx = self.manager_tx;
    let atx = tx.clone();
    let _ = tokio::spawn(async move {
        let mut serve_exit_tx: Option<oSender<()>> = None;
        let mut rx = rx;
        let tx = atx.clone();
        let rtx = reply_tx;
        let id = id;
        let sv = Sv{
            id: id,
            nickname: nickname.clone(),
            shared: shared.clone(),
            tx: atx.clone(),
        };
        loop {
            match rx.recv().await {
                Some(msg) => {
                    match msg {
                        Message::SendFile(p) => {
                            debug!("Got 'SendFile' Message: {}", &p);
                            let _ = rtx.send(Response::Done(Message::SendFile(p))).await;
                        },
                        Message::AddDir(d) => {
                            debug!("Got 'AddDir' Request: {:?}", &d);
                            let dir = d.to_string_lossy().to_string();
                            shared.write().unwrap().push(Directory{
                                name: dir.clone(),
                                paths: vec![dir]
                            });
                            let _ = rtx.send(Response::Done(Message::AddDir(d))).await;
                        },
                        Message::RemoveDir(name) => {
                            debug!("Got 'RemoveDir' Request: {:?}", &name);
                            shared.write().unwrap().retain(|d| d.name != name.to_string_lossy().to_string());
                            let _ = rtx.send(Response::Done(Message::RemoveDir(name))).await;
                        },
                        Message::SetNickname(n) => {
                            debug!("Got 'SetNickname' Request: {}", &n);
                            {                            
                                *nickname.write().unwrap() = n.clone();
                            }
                            let _ = rtx.send(Response::Done(Message::SetNickname(n))).await;
                        },
                        Message::StartBroadcast => {
                            debug!("Got 'StartBroadcast' Request");
                            let _ = rtx.send(Response::Done(Message::StartBroadcast)).await;
                        },
                        Message::StopBroadcast => {
                            debug!("Got 'StopBroadcast' Request");
                            let _ = rtx.send(Response::Done(Message::StartBroadcast)).await;
                        },
                        Message::StartListen => {
                            debug!("Got 'StartListen' Request");
                            let _ = rtx.send(Response::Done(Message::StartListen)).await;
                        },
                        Message::StopListen => {
                            debug!("Got 'StopListen' Request");
                            let _ = rtx.send(Response::Done(Message::StopListen)).await;
                        },
                        Message::StartServe(addr) => {
                            debug!("Got 'StartServe' Request: {:?}", &addr);

                            let sv = sv.clone();

                            serve_exit_tx = Some(serve(sv, addr.clone()));

                            let _ = rtx.clone().send(Response::Done(Message::StartServe(addr))).await;
                        },
                        Message::StopServe => {
                            debug!("Got 'StopServe' Request");
                            if let Some(t) = serve_exit_tx.take() {
                                match t.send(()) {
                                    Ok(_) => {
                                        let _ = rtx.send(Response::Done(Message::StopServe)).await;
                                    }
                                    Err(_) => {
                                        let _ = rtx.send(Response::Err("failed to stop serving".to_string())).await;
                                    }
                                };
                            }
                        },
                        Message::Exit => {
                            debug!("SENDING EXIT REQUESTS...");
                            let _ = tx.send(Message::StopListen).await;
                            debug!("SEND EXIT REQUEST: StopListen");
                            let _ = tx.send(Message::StopBroadcast).await;
                            debug!("SEND EXIT REQUEST: StopBroadcast");
                            let _ = tx.send(Message::StopServe).await;
                            debug!("SEND EXIT REQUEST: StopServe");
                            let _ = rtx.send(Response::Done(Message::Exit)).await;
                            break;
                        },
                    }
                },
                None => {
                }
            }
        }
    });
    (tx, reply_rx)
}

}

    
#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "DEBUG".to_string());
    let _ = env_logger::init();

    let s = SvBuilder::new("blub".to_string(), vec!["dir_1".to_string(), "dir_2".to_string()]);
    let (t,mut r) = s.run().await;
    let _ = t.send(Message::AddDir(PathBuf::from("C:/Test1"))).await;
    let _ = t.send(Message::RemoveDir(PathBuf::from("Test1"))).await;
    let _ = t.send(Message::SetNickname("Nick_123".to_string())).await;
    let _ = t.send(Message::StartServe(SocketAddr::new(Ipv4Addr::new(0,0,0,0).into(), 9001))).await;
    let _ = t.send(Message::StartBroadcast).await;
    let _ = t.send(Message::StartListen).await;

    thread::sleep(Duration::from_secs(20));

    let _ = t.send(Message::Exit).await;
    
    while let Some(msg) = r.recv().await {
        match msg {
            Response::Done(m) => {
                info!("DONE = {:?}", m);
            },
            Response::Err(e) => {
                info!("ERROR = {:?}", e);
            }
        };
    };
    Ok(())
}