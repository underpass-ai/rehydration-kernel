use std::sync::Arc;
use std::time::Instant;

use opentelemetry::KeyValue;
use rehydration_application::{
    ContextRenderOptions, GetContextPathQuery, GetContextQuery, GetNodeDetailQuery,
    QueryApplicationService, RehydrateSessionQuery, ValidateScopeQuery,
};
use rehydration_domain::{
    GraphNeighborhoodReader, NodeDetailReader, QualityMetricsObserver,
    QualityObservationContext, SnapshotStore,
};
use rehydration_proto::v1beta1::{
    GetContextPathRequest, GetContextPathResponse, GetContextRequest, GetContextResponse,
    GetNodeDetailRequest, GetNodeDetailResponse, RehydrateSessionRequest, RehydrateSessionResponse,
    ValidateScopeRequest, ValidateScopeResponse, context_query_service_server::ContextQueryService,
};
use tonic::{Request, Response, Status};

use crate::transport::proto_mapping_v1beta1::{
    proto_bundle_from_single_role_v1beta1, proto_graph_node_v1beta1,
    proto_node_detail_view_v1beta1, proto_rehydrate_session_response_v1beta1,
    proto_rendered_context_from_result_v1beta1, proto_rendered_context_v1beta1,
    proto_scope_validation_v1beta1, proto_timing_breakdown_v1beta1,
};
use crate::transport::support::map_application_error;

#[derive(Clone)]
pub struct QueryGrpcServiceV1Beta1<G, D, S> {
    application: Arc<QueryApplicationService<G, D, S>>,
    quality_observer: Arc<dyn QualityMetricsObserver>,
}

impl<G, D, S> QueryGrpcServiceV1Beta1<G, D, S> {
    pub fn new(
        application: Arc<QueryApplicationService<G, D, S>>,
        quality_observer: Arc<dyn QualityMetricsObserver>,
    ) -> Self {
        Self {
            application,
            quality_observer,
        }
    }
}

#[tonic::async_trait]
impl<G, D, S> ContextQueryService for QueryGrpcServiceV1Beta1<G, D, S>
where
    G: GraphNeighborhoodReader + Send + Sync + 'static,
    D: NodeDetailReader + Send + Sync + 'static,
    S: SnapshotStore + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self, request), fields(rpc = "GetContext"))]
    async fn get_context(
        &self,
        request: Request<GetContextRequest>,
    ) -> Result<Response<GetContextResponse>, Status> {
        let start = Instant::now();
        let request = request.into_inner();
        let requested_mode = map_proto_rehydration_mode(request.rehydration_mode);
        tracing::debug!(
            root_node_id = %request.root_node_id,
            role = %request.role,
            depth = request.depth,
            token_budget = request.token_budget,
            rehydration_mode = %requested_mode.as_str(),
            "handling get_context"
        );
        let result = self
            .application
            .get_context(GetContextQuery {
                root_node_id: request.root_node_id,
                role: request.role,
                depth: request.depth,
                render_options: ContextRenderOptions {
                    focus_node_id: None,
                    token_budget: (request.token_budget > 0).then_some(request.token_budget),
                    max_tier: map_proto_resolution_tier(request.max_tier),
                    rehydration_mode: requested_mode,
                },
                requested_scopes: request.requested_scopes,
            })
            .await
            .map_err(map_application_error)?;

        let meter = opentelemetry::global::meter("rehydration-kernel");
        let attrs = &[KeyValue::new("rpc", "GetContext")];
        meter
            .f64_histogram("rehydration.rpc.duration")
            .build()
            .record(start.elapsed().as_secs_f64(), attrs);
        meter
            .u64_histogram("rehydration.bundle.nodes")
            .build()
            .record(result.bundle.stats().selected_nodes() as u64, attrs);
        meter
            .u64_histogram("rehydration.bundle.relationships")
            .build()
            .record(result.bundle.stats().selected_relationships() as u64, attrs);
        meter
            .u64_histogram("rehydration.bundle.details")
            .build()
            .record(result.bundle.stats().detailed_nodes() as u64, attrs);
        meter
            .u64_histogram("rehydration.rendered.tokens")
            .build()
            .record(result.rendered.token_count as u64, attrs);
        if result.rendered.truncation.is_some() {
            meter
                .u64_counter("rehydration.truncation.total")
                .build()
                .add(1, attrs);
        }
        let resolved_mode = result.rendered.resolved_mode;
        meter.u64_counter("rehydration.mode.selected").build().add(
            1,
            &[
                KeyValue::new("rpc", "GetContext"),
                KeyValue::new("mode", resolved_mode.as_str().to_string()),
            ],
        );
        tracing::debug!(resolved_mode = %resolved_mode.as_str(), "mode resolved");

        // Quality metrics — delegated to observer (OTel + Loki/Tracing)
        self.quality_observer.observe(
            &result.rendered.quality,
            &QualityObservationContext {
                rpc: "GetContext".to_string(),
                root_node_id: result.bundle.root_node_id().as_str().to_string(),
                role: result.bundle.role().as_str().to_string(),
            },
        );

        if let Some(ref timing) = result.timing {
            meter
                .f64_histogram("rehydration.session.graph_load.duration")
                .build()
                .record(timing.graph_load.as_secs_f64(), attrs);
            meter
                .f64_histogram("rehydration.session.detail_load.duration")
                .build()
                .record(timing.detail_load.as_secs_f64(), attrs);
            meter
                .f64_histogram("rehydration.session.bundle_assembly.duration")
                .build()
                .record(timing.bundle_assembly.as_secs_f64(), attrs);
            meter
                .u64_histogram("rehydration.session.batch_size")
                .build()
                .record(timing.batch_size as u64, attrs);
        }

        Ok(Response::new(GetContextResponse {
            bundle: Some(proto_bundle_from_single_role_v1beta1(&result.bundle)),
            rendered: Some(proto_rendered_context_from_result_v1beta1(&result)),
            scope_validation: None,
            served_at: Some(crate::transport::support::timestamp_from(result.served_at)),
            timing: result.timing.as_ref().map(proto_timing_breakdown_v1beta1),
        }))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "GetContextPath"))]
    async fn get_context_path(
        &self,
        request: Request<GetContextPathRequest>,
    ) -> Result<Response<GetContextPathResponse>, Status> {
        let start = Instant::now();
        let request = request.into_inner();
        let obs_root = request.root_node_id.clone();
        let obs_role = request.role.clone();
        tracing::debug!(
            root_node_id = %request.root_node_id,
            target_node_id = %request.target_node_id,
            role = %request.role,
            "handling get_context_path"
        );
        let result = self
            .application
            .get_context_path(GetContextPathQuery {
                root_node_id: request.root_node_id,
                target_node_id: request.target_node_id,
                role: request.role,
                render_options: ContextRenderOptions {
                    focus_node_id: None,
                    token_budget: (request.token_budget > 0).then_some(request.token_budget),
                    max_tier: None,
                    rehydration_mode: rehydration_domain::RehydrationMode::default(),
                },
            })
            .await
            .map_err(map_application_error)?;

        let meter = opentelemetry::global::meter("rehydration-kernel");
        let attrs = &[KeyValue::new("rpc", "GetContextPath")];
        meter
            .f64_histogram("rehydration.rpc.duration")
            .build()
            .record(start.elapsed().as_secs_f64(), attrs);

        if let Some(ref timing) = result.timing {
            meter
                .f64_histogram("rehydration.session.graph_load.duration")
                .build()
                .record(timing.graph_load.as_secs_f64(), attrs);
            meter
                .f64_histogram("rehydration.session.detail_load.duration")
                .build()
                .record(timing.detail_load.as_secs_f64(), attrs);
            meter
                .f64_histogram("rehydration.session.bundle_assembly.duration")
                .build()
                .record(timing.bundle_assembly.as_secs_f64(), attrs);
            meter
                .u64_histogram("rehydration.session.batch_size")
                .build()
                .record(timing.batch_size as u64, attrs);
        }

        // Quality metrics — delegated to observer (OTel + Loki/Tracing)
        self.quality_observer.observe(
            &result.rendered.quality,
            &QualityObservationContext {
                rpc: "GetContextPath".to_string(),
                root_node_id: obs_root,
                role: obs_role,
            },
        );

        Ok(Response::new(GetContextPathResponse {
            path_bundle: Some(proto_bundle_from_single_role_v1beta1(&result.path_bundle)),
            rendered: Some(proto_rendered_context_v1beta1(&result.rendered, &[])),
            served_at: Some(crate::transport::support::timestamp_from(result.served_at)),
            timing: result.timing.as_ref().map(proto_timing_breakdown_v1beta1),
        }))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "GetNodeDetail"))]
    async fn get_node_detail(
        &self,
        request: Request<GetNodeDetailRequest>,
    ) -> Result<Response<GetNodeDetailResponse>, Status> {
        let request = request.into_inner();
        tracing::debug!(node_id = %request.node_id, "handling get_node_detail");
        let result = self
            .application
            .get_node_detail(GetNodeDetailQuery {
                node_id: request.node_id,
            })
            .await
            .map_err(map_application_error)?;

        Ok(Response::new(GetNodeDetailResponse {
            node: Some(proto_graph_node_v1beta1(&result.node)),
            detail: result.detail.as_ref().map(proto_node_detail_view_v1beta1),
        }))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "RehydrateSession"))]
    async fn rehydrate_session(
        &self,
        request: Request<RehydrateSessionRequest>,
    ) -> Result<Response<RehydrateSessionResponse>, Status> {
        let start = Instant::now();
        let request = request.into_inner();
        tracing::debug!(
            root_node_id = %request.root_node_id,
            roles = ?request.roles,
            persist_snapshot = request.persist_snapshot,
            "handling rehydrate_session"
        );
        // NOTE: `request.include_timeline` and `request.include_summaries` are
        // proto fields reserved for future use. They are intentionally not mapped
        // to application-layer queries in v1beta1.
        let snapshot_ttl_seconds = match (request.persist_snapshot, request.snapshot_ttl) {
            (true, None) => {
                return Err(Status::invalid_argument(
                    "snapshot_ttl is required when persist_snapshot is true",
                ));
            }
            (_, Some(d)) => d.seconds.max(0) as u64,
            (false, None) => 0,
        };
        let result = self
            .application
            .rehydrate_session(RehydrateSessionQuery {
                root_node_id: request.root_node_id,
                roles: request.roles,
                persist_snapshot: request.persist_snapshot,
                snapshot_ttl_seconds,
                timeline_window: request.timeline_window,
            })
            .await
            .map_err(map_application_error)?;

        let meter = opentelemetry::global::meter("rehydration-kernel");
        let attrs = &[KeyValue::new("rpc", "RehydrateSession")];
        meter
            .f64_histogram("rehydration.rpc.duration")
            .build()
            .record(start.elapsed().as_secs_f64(), attrs);

        if let Some(ref timing) = result.timing {
            meter
                .f64_histogram("rehydration.session.graph_load.duration")
                .build()
                .record(timing.graph_load.as_secs_f64(), attrs);
            meter
                .f64_histogram("rehydration.session.detail_load.duration")
                .build()
                .record(timing.detail_load.as_secs_f64(), attrs);
            meter
                .f64_histogram("rehydration.session.bundle_assembly.duration")
                .build()
                .record(timing.bundle_assembly.as_secs_f64(), attrs);
            meter
                .u64_histogram("rehydration.session.role_count")
                .build()
                .record(timing.role_count as u64, attrs);
            meter
                .u64_histogram("rehydration.session.batch_size")
                .build()
                .record(timing.batch_size as u64, attrs);
        }

        Ok(Response::new(proto_rehydrate_session_response_v1beta1(
            &result,
        )))
    }

    #[tracing::instrument(skip(self, request), fields(rpc = "ValidateScope"))]
    async fn validate_scope(
        &self,
        request: Request<ValidateScopeRequest>,
    ) -> Result<Response<ValidateScopeResponse>, Status> {
        let request = request.into_inner();
        let result = self.application.validate_scope(ValidateScopeQuery {
            required_scopes: request.required_scopes,
            provided_scopes: request.provided_scopes,
        });

        Ok(Response::new(ValidateScopeResponse {
            result: Some(proto_scope_validation_v1beta1(&result)),
        }))
    }
}

/// Maps proto `ResolutionTier` enum to domain. Returns `None` for UNSPECIFIED (= all tiers).
fn map_proto_resolution_tier(value: i32) -> Option<rehydration_domain::ResolutionTier> {
    match rehydration_proto::v1beta1::ResolutionTier::try_from(value) {
        Ok(rehydration_proto::v1beta1::ResolutionTier::L0Summary) => {
            Some(rehydration_domain::ResolutionTier::L0Summary)
        }
        Ok(rehydration_proto::v1beta1::ResolutionTier::L1CausalSpine) => {
            Some(rehydration_domain::ResolutionTier::L1CausalSpine)
        }
        Ok(rehydration_proto::v1beta1::ResolutionTier::L2EvidencePack) => {
            Some(rehydration_domain::ResolutionTier::L2EvidencePack)
        }
        _ => None, // UNSPECIFIED or unknown → all tiers
    }
}

/// Maps proto `RehydrationMode` enum to domain. UNSPECIFIED → Auto.
fn map_proto_rehydration_mode(value: i32) -> rehydration_domain::RehydrationMode {
    match rehydration_proto::v1beta1::RehydrationMode::try_from(value) {
        Ok(rehydration_proto::v1beta1::RehydrationMode::ResumeFocused) => {
            rehydration_domain::RehydrationMode::ResumeFocused
        }
        Ok(rehydration_proto::v1beta1::RehydrationMode::ReasonPreserving) => {
            rehydration_domain::RehydrationMode::ReasonPreserving
        }
        Ok(rehydration_proto::v1beta1::RehydrationMode::TemporalDelta) => {
            rehydration_domain::RehydrationMode::TemporalDelta
        }
        Ok(rehydration_proto::v1beta1::RehydrationMode::GlobalSummary) => {
            rehydration_domain::RehydrationMode::GlobalSummary
        }
        _ => rehydration_domain::RehydrationMode::Auto,
    }
}
