use self::pb::get_file_response::FileResponse;
use self::pb::{
    lan_doh_server::LanDoh, GetDirectoryRequest, GetDirectoryResponse, GetFileRequest,
    GetFileResponse, ListDirectoriesRequest, ListDirectoriesResponse,
};
use self::pb::{FileMetaData, HealthzRequest, HealthzResponse};

use super::order::Order;
use super::server::Server;
use super::service::Service;
use super::Request;

use std::pin::Pin;

use tokio::sync::mpsc::{self, Receiver, Sender};

use tokio_stream::{wrappers::ReceiverStream, Stream};

pub mod pb {
    tonic::include_proto!("pb");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("pb_descriptor");
}

#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};

#[tonic::async_trait]
impl LanDoh for Server {
    type GetFileStream = Pin<Box<dyn Stream<Item = Result<GetFileResponse, tonic::Status>> + Send>>;

    async fn healthz(
        &self,
        request: tonic::Request<HealthzRequest>,
    ) -> Result<tonic::Response<HealthzResponse>, tonic::Status> {
        self.send(Order::GotRequest(Request::new(Service::Healthz, request)))
            .await;

        Ok(tonic::Response::new(HealthzResponse {
            broadcaster: self.is_broadcasting().await,
            event_listener: self.is_event_listening().await,
            address: self.get_address(),
            id: self.get_id(),
            nickname: self.get_nickname().await,
        }))
    }

    async fn list_directories(
        &self,
        _request: tonic::Request<ListDirectoriesRequest>,
    ) -> Result<tonic::Response<ListDirectoriesResponse>, tonic::Status> {
        unimplemented!("SV::LIST_DIRECTORIES");
    }
    async fn get_directory(
        &self,
        _request: tonic::Request<GetDirectoryRequest>,
    ) -> Result<tonic::Response<GetDirectoryResponse>, tonic::Status> {
        unimplemented!("SV::GET_DIRECTORY");
    }

    async fn get_file(
        &self,
        request: tonic::Request<GetFileRequest>,
    ) -> Result<tonic::Response<Self::GetFileStream>, tonic::Status> {
        let (tx, rx): (
            Sender<Result<GetFileResponse, tonic::Status>>,
            Receiver<Result<GetFileResponse, tonic::Status>>,
        ) = mpsc::channel(128);

        info!("Got 'GetFile' Request: {:?}", request);

        tokio::spawn(async move {
            let path = "pseudo_path".to_string();
            let hash = "psuedo_hash".to_string();

            loop {
                let _ = tx
                    .send(Ok(GetFileResponse {
                        file_response: Some(FileResponse::Meta(FileMetaData {
                            file_size: 0,
                            path: path.clone(),
                            hash: hash.clone(),
                        })),
                    }))
                    .await;
            }
        });

        let output_stream: ReceiverStream<Result<GetFileResponse, tonic::Status>> =
            ReceiverStream::new(rx);

        Ok(tonic::Response::new(
            Box::pin(output_stream) as Self::GetFileStream
        ))
    }
}
