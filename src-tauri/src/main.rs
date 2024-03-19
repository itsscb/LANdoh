// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{net::SocketAddr, sync::Arc, thread, time::Duration};

use landoh::client::Client;

use landoh::app::{App, Config};

use landoh::source::Source;
use log::{info, warn};
use tauri::{Manager, Window};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(serde::Serialize, Debug, Clone)]
struct Payload {
    name: String,
    id: String,
    nickname: String,
    ip: Option<String>,
}

#[tauri::command]
async fn serve(state: tauri::State<'_, Arc<tokio::sync::Mutex<App>>>) -> Result<(), ()> {
    let a = Arc::clone(&state);
    tauri::async_runtime::spawn(async move {
        a.lock().await.serve().await;
    });
    Ok(())
}

#[tauri::command]
async fn request_dir(
    id: String,
    dir: String,
    state: tauri::State<'_, Arc<tokio::sync::Mutex<App>>>,
) -> Result<(), ()> {
    let a = Arc::clone(&state);
    tauri::async_runtime::spawn(async move {
        let mut addr = String::from("http://");
        let ip = match a
            .lock()
            .await
            .sources
            .lock()
            .await
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.ip.clone())
        {
            Some(Some(ip)) => ip,
            _ => "127.0.0.1".to_string(),
        };

        // // TODO: Remove for PROD
        // let ip = "127.0.0.1".to_string();

        addr.push_str(&ip);
        addr.push_str(":9001");

        let dest = a
            .lock()
            .await
            .config
            .lock()
            .await
            .destination
            .to_str()
            .unwrap()
            .to_string();

        let c = Arc::new(Client::new(dest));

        info!("REQUESTING: {} from {:?}", dir, addr);

        let files = c.get_directory(dir, addr.to_string()).await.unwrap();
        c.get_all_files(addr.to_string(), files).await.unwrap();
    });
    Ok(())
}

#[tauri::command]
async fn test_emit(
    state: tauri::State<'_, Arc<tokio::sync::Mutex<App>>>,
    window: Window,
) -> Result<(), ()> {
    let a = Arc::clone(&state);
    tauri::async_runtime::spawn(async move {
        let num = a.lock().await.sources.lock().await.len();
        let s = landoh::source::Source::new(
            format!("{}", num),
            format!("nick-{}", num),
            None,
            vec!["root".to_string()],
        );
        let _ = window.emit_all("test_emit", s);
    });
    Ok(())
}

#[tauri::command]
async fn listen(
    state: tauri::State<'_, Arc<tokio::sync::Mutex<App>>>,
    window: Window,
) -> Result<(), ()> {
    let a = Arc::clone(&state);
    // let wa = Arc::new(window);
    // let w = Arc::clone(&wa);
    tauri::async_runtime::spawn(async move {
        let rx = a.lock().await.listen().await;
        tauri::async_runtime::spawn(async move {
            while let Ok(s) = rx.recv() {
                let mut payload: Vec<Payload> = vec![];
                s.iter().for_each(|so| {
                    so.shared_directories.iter().for_each(|d| {
                        payload.push(Payload {
                            name: d.to_string(),
                            id: so.id.clone(),
                            nickname: so.nickname.clone(),
                            ip: so.ip.clone(),
                        });
                    })
                });

                info!("got update: {:?}", &payload);
                let err = window.emit_all("sources", payload);
                match err {
                    Ok(_) => {}
                    Err(err) => {
                        warn!("Error emitting message: {:?}", err);
                    }
                }
            }
        });
        // let _ = wa.emit_all("message", "SOURCES: listening for updates");
    });
    Ok(())
}

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
        TestBroadcast {
            #[arg(short, long)]
            nickname: Option<String>,
            #[arg(short, long)]
            id: Option<String>,
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
        Some(Commands::TestBroadcast { dirs, nickname, id }) => {
            let tx = landoh::multicast::Sender::new().unwrap();
            let mut def = vec!["root".to_string(), "testdir".to_string()];
            let dirs = match dirs {
                Some(mut d) => {
                    d.append(&mut def);
                    d
                }
                None => def,
            };

            let nick = match nickname {
                Some(n) => n,
                None => "test-nick".to_string(),
            };

            let uid = match id {
                Some(i) => i,
                None => Uuid::new_v4().to_string(),
            };

            let s = Source::new(uid, nick, None, dirs);

            info!("sending payload: {:?}", &s);

            let _ = tx.send(s);
        }
        Some(Commands::Serve { dirs, address }) => {
            let addr: SocketAddr = match address {
                Some(addr) => addr.as_str().parse()?,
                None => "0.0.0.0:9001".parse()?,
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
            app.listen().await;
            let s = Arc::clone(&app.sources);
            app.handles.spawn(async move {
                loop {
                    thread::sleep(Duration::from_secs(5));
                    info!("client dirs: {:?}", s.lock().await);
                }
            });
            app.broadcast().await;
            app.serve().await;
            // app.handles.spawn(async move {
            //     tauri::Builder::default()
            //         .run(tauri::generate_context!());
            //         .expect("error while running tauri application");
            // });
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
            let app = match App::new_from_config() {
                Ok(a) => a,
                Err(_) => {
                    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();

                    let config = Config::new(
                        vec![".".to_string(), "testdir".to_string()],
                        "testdestination".to_string(),
                        addr,
                        None,
                    )
                    .unwrap();
                    App::new(config)
                }
            };

            tauri::Builder::default()
                .manage(Arc::new(Mutex::new(app)))
                // .invoke_handler(tauri::generate_handler![listen])
                .invoke_handler(tauri::generate_handler![
                    serve,
                    test_emit,
                    listen,
                    request_dir
                ])
                .run(tauri::generate_context!())
                .expect("error while running tauri application");
        }
    };

    Ok(())
}
