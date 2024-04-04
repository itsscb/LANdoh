use std::{
    env,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};
#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

use landoh::server::{Order, OrderResponse, Server};
#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_LOG", "DEBUG".to_string());
    let _ = env_logger::init();

    let s = Server::builder()
        .set_nickname("MyNick".to_string())
        .add_share(PathBuf::from("C:\\temp"))
        // .set_serve_address(SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), 9001))
        .enable_eventlistening()
        .enable_broadcasting()
        .build();

    let (t, mut r) = s.run().await;
    // let _ = t.send(Order::AddDir(PathBuf::from("C:/Test1"))).await;
    // let _ = t.send(Order::RemoveDir(PathBuf::from("Test1"))).await;
    let _ = t.send(Order::SetNickname("Nick_123".to_string())).await;

    let _ = t
        .send(Order::StartServe(Some(SocketAddr::new(
            Ipv4Addr::new(0, 0, 0, 0).into(),
            50051,
        ))))
        .await;
    let _ = t.send(Order::StartBroadcast).await;
    let _ = t.send(Order::StartListen).await;

    // let _ = t.send(Order::Exit).await;

    while let Some(msg) = r.recv().await {
        match msg {
            OrderResponse::Done(m) => {
                info!("DONE = {:?}", m);
            }
            OrderResponse::Err(e) => {
                info!("ERROR = {:?}", e);
            }
        };
    }
    Ok(())
}
