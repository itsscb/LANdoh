#[allow(unused_imports)]
use std::{net::{Ipv4Addr, SocketAddr}, sync::{Arc, RwLock}};

use tokio::sync::mpsc::{self, Sender, Receiver};
use uuid::Uuid;

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};
 
#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

use crate::pb::Directory;

#[allow(dead_code)]
pub enum Message {
    AddDir(Directory),
    RemoveDir(String),
    UpdateNickname(String),
    StopBroadcast,
    StartBroadcast,
    StopServe,
    StartServe(SocketAddr),
    StartListen,
    StopListen,
    Exit
}

#[allow(dead_code)]
#[derive(Debug)]
struct Sv {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared_directories: Arc<RwLock<Vec<Directory>>>,
    tx: Sender<Message>,
}

impl Sv {
    #[allow(dead_code)]
    pub async fn send(&self, m: Message) -> Result<(),tokio::sync::mpsc::error::SendError<Message>> {
        self.tx.send(m).await
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SvBuilder {
    id: String,
    nickname: Arc<RwLock<String>>,
    shared_directories: Arc<RwLock<Vec<Directory>>>,
    manager_tx: Sender<Message>,
    manager_rx: Receiver<Message>
}

impl SvBuilder {
    #[allow(dead_code)]
    pub fn new(nickname: String, shared_directories: Vec<String>) -> Self {
        let (tx, rx) = mpsc::channel::<Message>(32);
        let nick = Arc::new(RwLock::new(nickname));
        let dirs = Arc::new(RwLock::new(shared_directories
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
            shared_directories: dirs,
        }      
    // manage(rx, nick.clone(), dirs.clone());
}
#[allow(dead_code)]
pub async fn run(self) -> (Sender<Message>, Receiver<String>){
    let (reply_tx, reply_rx) = mpsc::channel::<String>(32);

    let nickname = self.nickname;
    let shared_directories = self.shared_directories;
    let id = self.id;
    let rx = self.manager_rx;
    let tx = self.manager_tx;
    let _ = tokio::spawn(async move {
        let mut rx = rx;
        let tx = reply_tx;
        let id = id;
        loop {
            match rx.recv().await {
                Some(msg) => {
                    match msg {
                        Message::AddDir(d) => {
                            debug!("Adding Direcotry: {:?}", d);
                            let name = d.name.clone();
                            shared_directories.write().unwrap().push(d);
                            let _ = tx.send("Added dir: ".to_string() + &name).await;
                            // debug!("Directories: {:?}", a.lock().await.shared_directories);

                        },
                        Message::RemoveDir(d) => debug!("Removing Direcotry: {:?}", d),
                        Message::UpdateNickname(n) => {
                            debug!("Current Nickname: {:?}", nickname.read().unwrap());                            // debug!("New Nickname: {:?}", a.lock().await.nickname);
                            debug!("Updating Nickname: {}", n);
{                            *nickname.write().unwrap() = n;
}
debug!("New Nickname: {:?}", nickname.read().unwrap());                            // debug!("New Nickname: {:?}", a.lock().await.nickname);

                    },
                        Message::StartBroadcast => {
                            debug!("Starting Broadcast");
                            break;
                        },
                        Message::StopBroadcast => {
                            debug!("Stopping Broadcast");
                            break;
                        },
                        Message::StartListen => {
                            debug!("Starting Listen");
                            break;
                        },
                        Message::StopListen => {
                            debug!("Stopping Listen");
                            break;
                        },
                        Message::StartServe(addr) => {
                            debug!("Starting to Serve on {:?}", addr);
                            break;
                        },
                        Message::StopServe => {
                            debug!("Stopping to Serve");
                            break;
                        },
                        Message::Exit => {
                            debug!("Exiting");
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
            let _ = t.send(Message::AddDir(Directory{
                name: "Test1".to_string(),
                paths: vec!["Test1_Path".to_string()],
            })).await;
            let _ = t.send(Message::RemoveDir("Test1".to_string())).await;
            let _ = t.send(Message::UpdateNickname("Nick_123".to_string())).await;
            let _ = t.send(Message::StartServe(SocketAddr::new(Ipv4Addr::new(0,0,0,0).into(), 9001))).await;
            let _ = t.send(Message::Exit).await;
        });
    
    while let Some(msg) = r.recv().await {
        debug!("GOT = {}", msg);
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
//                             Message::UpdateNickname(n) => debug!("Updating Nickname: {}", n),
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