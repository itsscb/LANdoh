pub const PORT: u16 = 7645;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::Arc,
    time::Duration,
};

use tokio::sync::Mutex;

use serde::Serialize;

#[derive(Debug)]
pub struct Sender {
    socket: Arc<Mutex<UdpSocket>>,
    addr: SocketAddr,
}
impl Sender {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let ipv4: IpAddr = Ipv4Addr::new(224, 0, 0, 123).into();
        let addr = SocketAddr::new(ipv4, PORT);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        socket.set_multicast_if_v4(&Ipv4Addr::new(0, 0, 0, 0))?;

        socket.bind(&SockAddr::from(SocketAddr::new(
            Ipv4Addr::new(0, 0, 0, 0).into(),
            0,
        )))?;

        socket.set_read_timeout(Some(Duration::from_millis(100)))?;

        let socket: UdpSocket = socket.into();
        Ok(Sender {
            socket: Arc::new(Mutex::new(socket)),
            addr,
        })
    }

    pub async fn send<T: Serialize>(&self, data: T) -> Result<(), Box<dyn Error>> {
        let payload = serde_json::to_string(&data)?;

        self.socket
            .lock()
            .await
            .send_to(payload.as_bytes(), &self.addr)?;
        Ok(())
    }
}

pub mod receiver {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::{
        error::Error,
        io,
        net::{Ipv4Addr, SocketAddr, UdpSocket},
        sync::{mpsc::Sender, Arc},
        time::Duration,
    };

    use tokio::sync::Mutex;

    use log::error;

    pub use crate::source::Source;

    pub async fn listen(
        id: String,
        sources: Arc<Mutex<Vec<Source>>>,
        sender: Option<Sender<Vec<Source>>>,
    ) -> Result<(), Box<dyn Error>> {
        let ipv4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123).into();
        let addr = SocketAddr::new(ipv4.clone().into(), super::PORT);

        let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;

        socket.set_read_timeout(Some(Duration::from_millis(100)))?;
        socket.join_multicast_v4(&ipv4, &Ipv4Addr::UNSPECIFIED)?;
        bind_multicast(&socket, &addr)?;
        let listener: UdpSocket = socket.into();
        loop {
            let mut buf = [0u8; 1024];
            match listener.recv_from(&mut buf) {
                Ok((len, remote_addr)) => {
                    if remote_addr.to_string() == id {
                        ()
                    }
                    let d = &buf[..len];
                    let payload_raw = serde_json::from_slice::<Source>(d);
                    match payload_raw {
                        Ok(p) => {
                            if p.id == id {
                                continue;
                            }

                            let mut dirs = sources.lock().await;
                            match dirs.iter_mut().find(|ref i| i.id == p.id) {
                                Some(ref mut i) => {
                                    i.nickname = p.nickname;
                                    i.ip = Some(remote_addr.ip().to_string());
                                    i.shared_directories = p.shared_directories;
                                }
                                None => dirs.push(Source::new(
                                    p.id,
                                    p.nickname,
                                    Some(remote_addr.ip().to_string()),
                                    p.shared_directories,
                                )),
                            };

                            let _ = match sender {
                                Some(ref s) => s.send(dirs.iter().map(|d| d.clone()).collect()),
                                None => Ok(()),
                            };
                        }
                        Err(err) => {
                            error!("{}", err);
                        }
                    }
                }
                Err(_) => {}
            }
        }
        #[allow(unreachable_code)]
        Ok(())
    }

    #[cfg(windows)]

    fn bind_multicast(socket: &Socket, addr: &SocketAddr) -> io::Result<()> {
        let addr = SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), addr.port());
        socket.bind(&socket2::SockAddr::from(addr))
    }

    #[cfg(unix)]

    fn bind_multicast(socket: &Socket, addr: &SocketAddr) -> io::Result<()> {
        socket.bind(&socket2::SockAddr::from(*addr))
    }
}
