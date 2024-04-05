use std::{net::SocketAddr, path::PathBuf};

use super::Request;

#[allow(dead_code)]
#[derive(Debug)]
pub enum Order {
    GotRequest(Request),
    AddDir(PathBuf),
    SendFile(String),
    RemoveDir(PathBuf),
    SetNickname(String),
    StopBroadcast,
    StartBroadcast,
    StopServe,
    StartServe(Option<SocketAddr>),
    StartListen,
    StopListen,
    Exit,
}

#[derive(Debug)]
pub enum OrderResponse {
    Done(Order),
    Err(String),
}
