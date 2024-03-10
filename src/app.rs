mod model {
    include!("model.rs");
}

use self::model::Directory;

mod server {
    include!("server.rs");
}

use self::server::Server;

mod client {
    include!("client.rs");
}

use self::client::Client;

#[derive(Debug, Default)]
pub struct App {
    server: Server,
    client: Client,
}
