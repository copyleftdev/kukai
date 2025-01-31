use std::pin::Pin;
use std::sync::Arc;
use anyhow::Result;
use futures::{Stream, StreamExt};
use tokio::sync::{Mutex, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, transport::Server};
use arrow_flight::{
    flight_service_server::{FlightService, FlightServiceServer},
    Action, ActionType, Criteria, FlightData, FlightDescriptor, FlightInfo,
    HandshakeRequest, HandshakeResponse, PutResult, SchemaResult, Ticket,
};
use tracing::{info, warn};

use crate::config::{LoadConfig, CommanderConfig};

pub struct KaukaiFlightServer {
    pub data_store: Arc<Mutex<Vec<FlightData>>>,
}

impl KaukaiFlightServer {
    pub fn new() -> Self {
        Self {
            data_store: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

type PutResultStream = Pin<Box<dyn Stream<Item = Result<PutResult, Status>> + Send + 'static>>;
type EmptyStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + 'static>>;

#[tonic::async_trait]
impl FlightService for KaukaiFlightServer {
    type HandshakeStream = EmptyStream<HandshakeResponse>;
    type ListFlightsStream = EmptyStream<FlightInfo>;
    type DoGetStream = EmptyStream<FlightData>;
    type DoPutStream = PutResultStream;
    type DoExchangeStream = EmptyStream<FlightData>;
    type DoActionStream = EmptyStream<arrow_flight::Result>;
    type ListActionsStream = EmptyStream<ActionType>;

    async fn handshake(
        &self,
        req: Request<tonic::Streaming<HandshakeRequest>>,
    ) -> Result<Response<Self::HandshakeStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let mut stream = req.into_inner();

        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                info!("Commander handshake: {:?}", msg);
            }
            let _ = tx.send(Ok(HandshakeResponse {
                protocol_version: 0,
                payload: Vec::new().into(),
            }))
            .await;
        });
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn list_flights(
        &self,
        _: Request<Criteria>,
    ) -> Result<Response<Self::ListFlightsStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(Err(Status::unimplemented("list_flights unimplemented"))).await;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn do_get(
        &self,
        _: Request<Ticket>,
    ) -> Result<Response<Self::DoGetStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(Err(Status::unimplemented("do_get unimplemented"))).await;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn do_put(
        &self,
        request: Request<tonic::Streaming<FlightData>>,
    ) -> Result<Response<Self::DoPutStream>, Status> {
        let (tx, rx) = mpsc::channel(2);
        let store_ref = self.data_store.clone();
        let mut inbound = request.into_inner();

        tokio::spawn(async move {
            while let Some(Ok(chunk)) = inbound.next().await {
                {
                    let mut guard = store_ref.lock().await;
                    guard.push(chunk);
                }
                if let Err(e) = tx.send(Ok(PutResult {
                    app_metadata: Vec::new().into(),
                })).await {
                    warn!("Commander: do_put send error: {:?}", e);
                }
            }
        });

        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn do_exchange(
        &self,
        _: Request<tonic::Streaming<FlightData>>,
    ) -> Result<Response<Self::DoExchangeStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(Err(Status::unimplemented("do_exchange unimplemented"))).await;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn do_action(
        &self,
        _: Request<Action>,
    ) -> Result<Response<Self::DoActionStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(Err(Status::unimplemented("do_action unimplemented"))).await;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn list_actions(
        &self,
        _: Request<arrow_flight::Empty>,
    ) -> Result<Response<Self::ListActionsStream>, Status> {
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(Err(Status::unimplemented("list_actions unimplemented"))).await;
        Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
    }

    async fn get_flight_info(
        &self,
        _: Request<FlightDescriptor>,
    ) -> Result<Response<FlightInfo>, Status> {
        Err(Status::unimplemented("get_flight_info unimplemented"))
    }

    async fn get_schema(
        &self,
        _: Request<FlightDescriptor>,
    ) -> Result<Response<SchemaResult>, Status> {
        Err(Status::unimplemented("get_schema unimplemented"))
    }
}

pub async fn run_commander(load: &LoadConfig, cmdr: &CommanderConfig) -> Result<()> {
    info!("Commander edges: {:?}", cmdr.edges);
    info!("Commander ignoring RPS: {}", load.rps);

    let addr = "0.0.0.0:50051".parse()?;
    let service = KaukaiFlightServer::new();
    info!("Commander listening on {}", addr);

    Server::builder()
        .add_service(FlightServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
