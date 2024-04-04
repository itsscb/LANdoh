use self::pb::get_file_response::FileResponse;
use self::pb::{
    lan_doh_server::LanDoh, GetFileRequest, GetFileResponse, GetGameRequest, GetGameResponse,
    ListGamesRequest, ListGamesResponse,
};
use self::pb::{FileMetaData, HealthzRequest, HealthzResponse};

use super::order::Order;
use super::server::Server;
use super::service::Service;
use super::Request;

use std::pin::Pin;

use tokio::sync::mpsc::{self, Receiver, Sender};

use tokio_stream::{wrappers::ReceiverStream, Stream};

pub(super) mod pb {
    tonic::include_proto!("proto");
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
    type GetGameStream = Pin<Box<dyn Stream<Item = Result<GetGameResponse, tonic::Status>> + Send>>;

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

    async fn list_games(
        &self,
        request: tonic::Request<ListGamesRequest>,
    ) -> Result<tonic::Response<ListGamesResponse>, tonic::Status> {
        self.send(Order::GotRequest(Request::new(
            Service::ListDirectories,
            request,
        )))
        .await;

        Ok(tonic::Response::new(ListGamesResponse {
            games: self.list_games().await,
        }))
    }
    async fn get_game(
        &self,
        _request: tonic::Request<GetGameRequest>,
    ) -> Result<tonic::Response<Self::GetGameStream>, tonic::Status> {
        // ) -> Result<tonic::Response<GetGameResponse>, tonic::Status> {
        unimplemented!("SV::GET_DIRECTORY");
    }

    async fn get_file(
        &self,
        request: tonic::Request<GetFileRequest>,
    ) -> Result<tonic::Response<Self::GetFileStream>, tonic::Status> {
        unimplemented!("TODO::GET_FILE");
        let (tx, rx): (
            Sender<Result<GetFileResponse, tonic::Status>>,
            Receiver<Result<GetFileResponse, tonic::Status>>,
        ) = mpsc::channel(128);

        info!("Got 'GetFile' Request: {:?}", request);

        // tokio::spawn(async move {
        //     let path = "pseudo_path".to_string();
        //     let hash = "psuedo_hash".to_string();

        //     loop {
        //         let _ = tx
        //             .send(Ok(GetFileResponse {
        //                 file_response: Some(FileResponse::Meta(FileMetaData {
        //                     file_size: 0,
        //                     path: path.clone(),
        //                     hash: hash.clone(),
        //                 })),
        //             }))
        //             .await;
        //     }
        // });

        let output_stream: ReceiverStream<Result<GetFileResponse, tonic::Status>> =
            ReceiverStream::new(rx);

        Ok(tonic::Response::new(
            Box::pin(output_stream) as Self::GetFileStream
        ))
    }
}
