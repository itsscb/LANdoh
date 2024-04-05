use core::fmt;
use std::net::SocketAddr;

use super::Service;

#[derive(Debug)]
pub struct Request {
    pub remote_address: Option<SocketAddr>,
    pub service: Service,
}

impl Request {
    pub fn new<T>(service: Service, request: tonic::Request<T>) -> Self {
        Self {
            remote_address: request.remote_addr(),
            service: service,
        }
    }
}

impl fmt::Display for Request {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.remote_address {
            Some(addr) => {
                write!(
                    f,
                    "{} requested {}",
                    addr.to_string(),
                    self.service.to_string()
                )
            }
            None => {
                write!(f, "UNKNOWN requested {}", self.service.to_string())
            }
        }
    }
}
