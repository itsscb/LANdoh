use super::order::Order;
use super::server::Server;
use super::service::Service;
use super::Request;

use std::pin::Pin;

use tokio_stream::Stream;

pub(super) mod pb {
    tonic::include_proto!("landoh");
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("pb_descriptor");
}

use self::pb::{
    games_service_server::GamesService, GetGameRequest, GetGameResponse, HealthzRequest,
    HealthzResponse, ListGamesRequest, ListGamesResponse,
};

#[allow(unused_imports)]
#[cfg(test)]
use std::{println as info, println as warn, println as debug, println as error};

#[allow(unused_imports)]
#[cfg(not(test))]
use log::{debug, error, info, warn};

#[tonic::async_trait]
impl GamesService for Server {
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
}
