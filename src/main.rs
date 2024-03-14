use std::{net::SocketAddr, sync::Arc, thread, time::Duration};

use landoh::client::Client;

use landoh::app::{App, Config};

use log::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::{Parser, Subcommand};

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        command: Option<Commands>,
    }

    #[derive(Subcommand)]
    enum Commands {
        Serve {
            #[arg(short, long)]
            address: Option<String>,
            #[arg(short, long, num_args(0..))]
            dirs: Option<Vec<String>>,
        },
        GetAllFiles {
            #[arg(short, long)]
            source: String,
            #[arg(short, long)]
            port: Option<String>,
            #[arg(long)]
            dir: String,
            #[arg(short, long)]
            destination: Option<String>,
        },
        ListDirectories {
            #[arg(short, long)]
            source: String,
            #[arg(short, long)]
            port: Option<String>,
        },
    }

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { dirs, address }) => {
            let addr: SocketAddr = match address {
                Some(addr) => addr.as_str().parse()?,
                None => "127.0.0.1:9001".parse()?,
            };
            let dirs = match dirs {
                Some(dirs) => dirs,
                None => vec![".".to_string()],
            };

            let config = Config::new(dirs, "testdestination".to_string(), addr, None).unwrap();

            let mut app = match App::new_from_config() {
                Ok(a) => a,
                Err(_) => App::new(config),
            };

            let _ = app.add_shared_dir("src".to_string(), vec!["src".to_string()]);
            app.listen();
            let s = Arc::clone(&app.sources);
            app.handles.spawn(async move {
                loop {
                    thread::sleep(Duration::from_secs(5));
                    info!("client dirs: {:?}", s.lock().unwrap());
                }
            });
            app.broadcast();
            app.serve().await;
            app.join_all().await;
        }
        Some(Commands::GetAllFiles {
            source,
            port,
            dir,
            destination,
        }) => {
            let mut addr = String::from("http://");
            addr.push_str(&source);
            match port {
                Some(p) => addr.push_str(&p),
                None => addr.push_str(":9001"),
            };
            let dest = match destination {
                Some(d) => d,
                None => ".".to_string(),
            };

            let c = Arc::new(Client::new(dest));

            let files = c.get_directory(dir, addr.to_string()).await.unwrap();
            c.get_all_files(addr.to_string(), files).await.unwrap();
        }
        Some(Commands::ListDirectories { source, port }) => {
            let mut addr = String::from("http://");
            addr.push_str(&source);
            match port {
                Some(p) => addr.push_str(&p),
                None => addr.push_str(":9001"),
            };

            let c = Arc::new(Client::new(String::from(".")));

            c.list_directories(addr).await?;
        }
        _ => {
            return Ok(());
        }
    };

    Ok(())
}
