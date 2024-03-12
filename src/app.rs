mod server {
    include!("server.rs");
}

pub use self::server::{Directory, Server};

pub struct App {
    pub server: Server,
}
