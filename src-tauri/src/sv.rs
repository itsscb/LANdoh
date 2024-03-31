use std::{error::Error, future::IntoFuture, path::PathBuf, pin::Pin};
#[allow(unused_imports)]
use std::{net::{Ipv4Addr, SocketAddr}, sync::{Arc, RwLock}};

use tokio::sync::{mpsc::{self, Receiver, Sender}, Mutex};
use tokio::sync::oneshot::{self, Sender as oSender, Receiver as oReceiver};
use tokio_stream::Stream;
use tonic::{async_trait, Request as tRequest, Response as tResponse, Status, transport::Server as tServer};
use uuid::Uuid;

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};
 
#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

use self::pb::{Directory, lan_doh_server::LanDoh, GetDirectoryRequest, GetDirectoryResponse, GetFileRequest, GetFileResponse, ListDirectoriesRequest, ListDirectoriesResponse};

mod pb {
    tonic::include_proto!("pb");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("pb_descriptor");
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Message {
    AddDir(PathBuf),
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
struct Sv {
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
        debug!("SV::LIST_DIRECTORIES");
        Ok(tResponse::new(ListDirectoriesResponse {
            dirs: vec![] }))
        }
    async fn get_directory(
        &self,
        request: tRequest<GetDirectoryRequest>,
    ) -> Result<tResponse<GetDirectoryResponse>, Status> {
        Ok(tResponse::new(GetDirectoryResponse{files: vec![]}))
        // unimplemented!("SV::GET_DIRECTORY");
    }

    async fn get_file(
        &self,
        request: tRequest<GetFileRequest>,
    ) -> Result<tResponse<Self::GetFileStream>, Status> {
        unimplemented!("SV::GET_FILE");
    }
}


#[allow(dead_code)]
    pub async fn serve(sv: Sv, addr: SocketAddr, exit: oReceiver<()>) -> Result<(), ()> {
        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(pb::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        let res = tServer::builder()
            .add_service(pb::lan_doh_server::LanDohServer::new(sv))
            .add_service(reflection_service)
            .serve_with_shutdown(addr, async { drop(exit) })
            // .serve(addr)
            .await;

        debug!("SERVE: {:?}", res);
        Ok(())
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
        let (tx, rx) = mpsc::channel::<Message>(32);
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
    let (reply_tx, reply_rx) = mpsc::channel::<Response>(32);

    let nickname = self.nickname;
    let shared = self.shared;
    let id = self.id;
    let rx = self.manager_rx;
    let tx = self.manager_tx;
    let atx = tx.clone();
    let _ = tokio::spawn(async move {
        // let mut serve_exit_tx: Arc<Mutex<Option<oSender<()>>>> = Arc::new(Mutex::new(None));
        // let serve_exit_fn = None;
        let mut rx = rx;
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
                        Message::AddDir(d) => {
                            let dir = d.to_string_lossy().to_string();
                            // debug!("Adding Direcotry: {:?}", d);
                            shared.write().unwrap().push(Directory{
                                name: dir.clone(),
                                paths: vec![dir]
                            });
                            let _ = rtx.send(Response::Done(Message::AddDir(d))).await;
                        },
                        Message::RemoveDir(name) => {
                            // debug!("Removing Direcotry: {:?}", &name);
                            shared.write().unwrap().retain(|d| d.name != name.to_string_lossy().to_string());
                            let _ = rtx.send(Response::Done(Message::RemoveDir(name))).await;
                        },
                        Message::SetNickname(n) => {
                            // debug!("Current Nickname: {:?}", nickname.read().unwrap());                            // debug!("New Nickname: {:?}", a.lock().await.nickname);
                            // debug!("Updating Nickname: {}", n);
                            {                            
                                *nickname.write().unwrap() = n.clone();
                            }
                            let _ = rtx.send(Response::Done(Message::SetNickname(n))).await;
                        },
                        Message::StartBroadcast => {
                            // debug!("Starting Broadcast");
                            let _ = rtx.send(Response::Done(Message::StartBroadcast)).await;
                        },
                        Message::StopBroadcast => {
                            // debug!("Stopping Broadcast");
                            let _ = rtx.send(Response::Done(Message::StartBroadcast)).await;
                        },
                        Message::StartListen => {
                            // debug!("Starting Listen");
                            let _ = rtx.send(Response::Done(Message::StartListen)).await;
                        },
                        Message::StopListen => {
                            // debug!("Stopping Listen");
                            let _ = rtx.send(Response::Done(Message::StopListen)).await;
                        },
                        Message::StartServe(addr) => {
                            let (s_exit_tx, serve_exit_rx) = oneshot::channel::<()>();
                            let rt = rtx.clone();
                            let sv = sv.clone();
                            tokio::spawn(async move {

                                let var_name = match serve(sv, addr, serve_exit_rx).await {
                                    Ok(_) => rt.send(Response::Done(Message::StartServe(addr))).await,
                                    Err(err) => rt.send(Response::Err("failed to serve".to_string())).await,
                                };
                            });
                            // *serve_exit_tx.lock().await = Some(s_exit_tx); 
                            // serve_exit_fn = Some(async move {
                            //     s_exit_tx.send(())
                            // }.into_future());
                        },
                        Message::StopServe => {
                            // if serve_exit_tx.lock().await.is_some() {
                            //     let mut t = serve_exit_tx.lock().await;
                            //     let mut t = t.as_mut().unwrap();
                            // // }
                            // // if let Some(t) = serve_exit_tx {
                            //     match t.send(()) {
                            //         Ok(_) => {
                            //             *serve_exit_tx.lock().await = None;
                            //             let _ = rtx.send(Response::Done(Message::StopServe)).await;
                            //         },
                            //         Err(e) => {
                            //             let _ = rtx.send(Response::Err(format!("{}: {:?}","failed to stop serving".to_string(), e))).await;
                            //         }
                            //     };
                            // }
                            // let t = serve_exit_tx.unwrap();
                            // t.send(());

                            // match serve_exit_tx {
                            //     Some(t) => {
                            //     },
                            //     None => {
                            //         let _ = rtx.send(Response::Err("currently not serving".to_string())).await;
                            //     },
                            // }
                            // match serve_exit_fn {
                            //     Some(ref f) => {
                            //         match f.await {
                            //             Ok(_) => {
                            //                 let _ = rtx.send(Response::Done(Message::StopServe)).await;
                            //             }
                            //             Err(e) => {

                            //             }
                            //         }
                            //     }
                            //     None => {}
                            // }
                        },
                        Message::Exit => {
                            // debug!("Exiting");
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

#[tokio::test]
async fn test_sv_new() {

    let s = SvBuilder::new("blub".to_string(), vec!["dir_1".to_string(), "dir_2".to_string()]);
    let (t,mut r) = s.run().await;
    // let t = s.manager_tx.clone();
    let v = tokio::spawn(async move {
            let _ = t.send(Message::AddDir(PathBuf::from("C:/Test1"))).await;
            let _ = t.send(Message::RemoveDir(PathBuf::from("Test1"))).await;
            let _ = t.send(Message::SetNickname("Nick_123".to_string())).await;
            let _ = t.send(Message::StartServe(SocketAddr::new(Ipv4Addr::new(0,0,0,0).into(), 9001))).await;
            let _ = t.send(Message::StopServe).await;
            let _ = t.send(Message::StartBroadcast).await;
            let _ = t.send(Message::StartBroadcast).await;
            let _ = t.send(Message::StartListen).await;
            let _ = t.send(Message::StopListen).await;
            // let _ = t.send(Message::Exit).await;
        });
    
    while let Some(msg) = r.recv().await {
        match msg {
            Response::Done(m) => {
                debug!("DONE = {:?}", m);
            },
            Response::Err(e) => {
                debug!("ERROR = {:?}", e);
            }
        };
    }
    v.await.unwrap();

    }

// struct Manager {
//     rx: Receiver<Message>,
//     // serve_tx: Sender<Message>,
//     // broadcast_tx: Sender<Message>
// }

// impl Manager {
//     pub fn new() -> Sender<Message> {
//         let (tx, mut rx) = mpsc::channel::<Message>(32);

//         let _ = tokio::spawn(async move {
//             loop {
//                 match rx.recv().await {
//                     Some(msg) => {
//                         match msg {
//                             Message::AddDir(d) => debug!("Adding Direcotry: {:?}", d),
//                             Message::RemoveDir(d) => debug!("Removing Direcotry: {:?}", d),
//                             Message::SetNickname(n) => debug!("Updating Nickname: {}", n),
//                             Message::Exit => {
//                                 debug!("Exiting");
//                                 break;
//                             },
//                         }
//                     },
//                     None => {
//                     }
//                 }
//             }
//         });

//         tx
//     }
// }