use std::sync::Arc;

use rehydration_application::KernelMemoryApplicationService;
use rehydration_domain::{
    ContextEventStore, GraphNeighborhoodReader, MemoryAboutIndexReader, NodeDetailReader,
    ProjectionWriter, SnapshotStore, TemporalDirection,
};
use rehydration_proto::v1beta1::{
    AskRequest, AskResponse, IngestRequest, IngestResponse, InspectRequest, InspectResponse,
    TemporalMoveRequest, TemporalMoveResponse, TemporalNearRequest, TraceRequest, TraceResponse,
    WakeRequest, WakeResponse, kernel_memory_service_server::KernelMemoryService,
};
use tonic::{Request, Response, Status};

use crate::transport::proto_mapping_v1beta1::{
    ask_query_from_proto, ask_response_from_result, ingest_command_from_proto,
    ingest_response_from_outcome, inspect_query_from_proto, inspect_response_from_result,
    temporal_query_from_move_proto, temporal_query_from_near_proto, temporal_response_from_result,
    trace_query_from_proto, trace_response_from_result, wake_query_from_proto,
    wake_response_from_result,
};
use crate::transport::support::map_application_error;

pub struct MemoryGrpcServiceV1Beta1<G, D, S, E, W> {
    application: Arc<KernelMemoryApplicationService<G, D, S, E, W>>,
}

impl<G, D, S, E, W> MemoryGrpcServiceV1Beta1<G, D, S, E, W> {
    pub fn new(application: Arc<KernelMemoryApplicationService<G, D, S, E, W>>) -> Self {
        Self { application }
    }
}

#[tonic::async_trait]
impl<G, D, S, E, W> KernelMemoryService for MemoryGrpcServiceV1Beta1<G, D, S, E, W>
where
    G: GraphNeighborhoodReader + MemoryAboutIndexReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
    E: ContextEventStore + Send + Sync + 'static,
    W: ProjectionWriter + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Ingest"))]
    async fn ingest(
        &self,
        request: Request<IngestRequest>,
    ) -> Result<Response<IngestResponse>, Status> {
        let command = ingest_command_from_proto(request.into_inner()).map_err(|status| *status)?;
        let outcome = self
            .application
            .ingest(command)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(ingest_response_from_outcome(outcome)))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Wake"))]
    async fn wake(&self, request: Request<WakeRequest>) -> Result<Response<WakeResponse>, Status> {
        let request = request.into_inner();
        let query = wake_query_from_proto(request.clone()).map_err(|status| *status)?;
        let intent = query.intent.clone();
        let result = self
            .application
            .wake(query)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(wake_response_from_result(&intent, result)))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Ask"))]
    async fn ask(&self, request: Request<AskRequest>) -> Result<Response<AskResponse>, Status> {
        let request = request.into_inner();
        let question = request.question.clone();
        let query = ask_query_from_proto(request).map_err(|status| *status)?;
        let answer_policy = query.answer_policy;
        let result = self
            .application
            .ask(query)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(ask_response_from_result(
            &question,
            answer_policy,
            result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Goto"))]
    async fn goto(
        &self,
        request: Request<TemporalMoveRequest>,
    ) -> Result<Response<TemporalMoveResponse>, Status> {
        self.temporal_move(request.into_inner(), TemporalDirection::Goto)
            .await
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Near"))]
    async fn near(
        &self,
        request: Request<TemporalNearRequest>,
    ) -> Result<Response<TemporalMoveResponse>, Status> {
        let request = request.into_inner();
        let requested_cursor = request.around.clone().unwrap_or_default();
        let query = temporal_query_from_near_proto(request).map_err(|status| *status)?;
        let result = self
            .application
            .temporal(query)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(temporal_response_from_result(
            requested_cursor,
            TemporalDirection::Near,
            result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Rewind"))]
    async fn rewind(
        &self,
        request: Request<TemporalMoveRequest>,
    ) -> Result<Response<TemporalMoveResponse>, Status> {
        self.temporal_move(request.into_inner(), TemporalDirection::Rewind)
            .await
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Forward"))]
    async fn forward(
        &self,
        request: Request<TemporalMoveRequest>,
    ) -> Result<Response<TemporalMoveResponse>, Status> {
        self.temporal_move(request.into_inner(), TemporalDirection::Forward)
            .await
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Trace"))]
    async fn trace(
        &self,
        request: Request<TraceRequest>,
    ) -> Result<Response<TraceResponse>, Status> {
        let query = trace_query_from_proto(request.into_inner());
        let result = self
            .application
            .trace(query)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(trace_response_from_result(result)))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "KernelMemory.Inspect"))]
    async fn inspect(
        &self,
        request: Request<InspectRequest>,
    ) -> Result<Response<InspectResponse>, Status> {
        let query = inspect_query_from_proto(request.into_inner()).map_err(|status| *status)?;
        let include_details = query.include_details;
        let result = self
            .application
            .inspect(query)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(inspect_response_from_result(
            result,
            include_details,
        )))
    }
}

impl<G, D, S, E, W> MemoryGrpcServiceV1Beta1<G, D, S, E, W>
where
    G: GraphNeighborhoodReader + MemoryAboutIndexReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
    E: ContextEventStore + Send + Sync + 'static,
    W: ProjectionWriter + Send + Sync + 'static,
{
    async fn temporal_move(
        &self,
        request: TemporalMoveRequest,
        direction: TemporalDirection,
    ) -> Result<Response<TemporalMoveResponse>, Status> {
        let requested_cursor = request.cursor.clone().unwrap_or_default();
        let query = temporal_query_from_move_proto(request, direction).map_err(|status| *status)?;
        let result = self
            .application
            .temporal(query)
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(temporal_response_from_result(
            requested_cursor,
            direction,
            result,
        )))
    }
}
