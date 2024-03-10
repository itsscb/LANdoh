mod server {
    include!("server.rs");
}

pub use self::server::Server;

pub struct App {
    pub server: Server,
}
