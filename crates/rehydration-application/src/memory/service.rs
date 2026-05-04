use std::collections::BTreeSet;
use std::sync::Arc;

use rehydration_domain::{
    ContextEventStore, DimensionSelection, DimensionSelectionMode, GraphNeighborhoodReader,
    NodeDetailReader, ProjectionWriter, RehydrationBundle, RehydrationMode, ResolutionTier,
    SnapshotStore, TemporalMemoryTraversal, TemporalTraversalRequest,
};

use crate::ApplicationError;
use crate::commands::CommandApplicationService;
use crate::memory::{
    AskMemoryQuery, ExistingMemoryRefs, InspectMemoryQuery, MemoryIngestCommand,
    MemoryIngestOutcome, TemporalMemoryQuery, TemporalMemoryResult, TraceMemoryQuery,
    WakeMemoryQuery, translate_memory_ingest,
};
use crate::queries::{
    ContextRenderOptions, EndpointHint, GetContextPathQuery, GetContextPathResult, GetContextQuery,
    GetContextResult, GetNodeDetailQuery, GetNodeDetailResult, MAX_NATIVE_GRAPH_TRAVERSAL_DEPTH,
    QueryApplicationService, render_graph_bundle_with_options,
};

pub struct KernelMemoryApplicationService<G, D, S, E, W> {
    query_application: Arc<QueryApplicationService<G, D, S>>,
    command_application: Arc<CommandApplicationService<E, W>>,
}

impl<G, D, S, E, W> KernelMemoryApplicationService<G, D, S, E, W> {
    pub fn new(
        query_application: Arc<QueryApplicationService<G, D, S>>,
        command_application: Arc<CommandApplicationService<E, W>>,
    ) -> Self {
        Self {
            query_application,
            command_application,
        }
    }
}

impl<G, D, S, E, W> KernelMemoryApplicationService<G, D, S, E, W>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
    S: SnapshotStore + Send + Sync,
    E: ContextEventStore + Send + Sync,
    W: ProjectionWriter + Send + Sync,
{
    pub async fn ingest(
        &self,
        command: MemoryIngestCommand,
    ) -> Result<MemoryIngestOutcome, ApplicationError> {
        let (update_context, mut outcome) =
            match translate_memory_ingest(&command, &ExistingMemoryRefs::default()) {
                Ok(translated) => translated,
                Err(error)
                    if is_unknown_memory_ref_validation(&error)
                        || command.memory.dimensions.is_empty() =>
                {
                    let existing = self.existing_memory_refs(&command.about).await?;
                    translate_memory_ingest(&command, &existing)?
                }
                Err(error) => return Err(error),
            };
        if command.dry_run {
            outcome
                .warnings
                .push("dry_run=true; validated memory without writing to the kernel".to_string());
            return Ok(outcome);
        }

        let accepted = self
            .command_application
            .update_context(update_context)
            .await?;
        outcome.read_after_write_ready = true;
        outcome.warnings = accepted.warnings;
        Ok(outcome)
    }

    pub async fn wake(&self, query: WakeMemoryQuery) -> Result<GetContextResult, ApplicationError> {
        let render_options = memory_render_options(
            query.token_budget,
            query.max_tier,
            RehydrationMode::ResumeFocused,
            EndpointHint::Neighborhood,
        );
        let result = self
            .query_application
            .get_context(GetContextQuery {
                root_node_id: query.about,
                role: query.role,
                depth: query.depth,
                requested_scopes: requested_dimension_scopes(&query.dimensions),
                render_options: render_options.clone(),
            })
            .await?;
        apply_dimension_selection(result, &query.dimensions, &render_options)
    }

    pub async fn ask(&self, query: AskMemoryQuery) -> Result<GetContextResult, ApplicationError> {
        let render_options = memory_render_options(
            query.token_budget,
            query.max_tier,
            RehydrationMode::ReasonPreserving,
            EndpointHint::Neighborhood,
        );
        let result = self
            .query_application
            .get_context(GetContextQuery {
                root_node_id: query.about,
                role: "answerer".to_string(),
                depth: query.depth,
                requested_scopes: requested_dimension_scopes(&query.dimensions),
                render_options: render_options.clone(),
            })
            .await?;
        apply_dimension_selection(result, &query.dimensions, &render_options)
    }

    pub async fn temporal(
        &self,
        query: TemporalMemoryQuery,
    ) -> Result<TemporalMemoryResult, ApplicationError> {
        let context = self
            .query_application
            .get_context(GetContextQuery {
                root_node_id: query.about,
                role: "temporal-reader".to_string(),
                depth: query.depth,
                requested_scopes: Vec::new(),
                render_options: memory_render_options(
                    query.token_budget,
                    query.max_tier,
                    RehydrationMode::ReasonPreserving,
                    EndpointHint::Neighborhood,
                ),
            })
            .await?;

        let request = TemporalTraversalRequest::new(query.direction, query.cursor)
            .with_dimensions(query.dimensions)
            .with_window(query.window);
        let request = if let Some(limit_entries) = query.limit_entries {
            request.with_limit_entries(limit_entries)?
        } else {
            request
        };

        let traversal = TemporalMemoryTraversal::traverse(&context.bundle, &request)?;

        Ok(TemporalMemoryResult {
            traversal,
            source_bundle: context.bundle,
            include: query.include,
        })
    }

    pub async fn trace(
        &self,
        query: TraceMemoryQuery,
    ) -> Result<GetContextPathResult, ApplicationError> {
        self.query_application
            .get_context_path(GetContextPathQuery {
                root_node_id: query.from,
                target_node_id: query.to,
                role: query.role,
                render_options: ContextRenderOptions {
                    focus_node_id: None,
                    token_budget: (query.token_budget > 0).then_some(query.token_budget),
                    max_tier: Some(ResolutionTier::L2EvidencePack),
                    rehydration_mode: RehydrationMode::ReasonPreserving,
                    endpoint_hint: EndpointHint::FocusedPath,
                },
            })
            .await
    }

    pub async fn inspect(
        &self,
        query: InspectMemoryQuery,
    ) -> Result<GetNodeDetailResult, ApplicationError> {
        self.query_application
            .get_node_detail(GetNodeDetailQuery {
                node_id: query.ref_id,
            })
            .await
    }

    async fn existing_memory_refs(
        &self,
        about: &str,
    ) -> Result<ExistingMemoryRefs, ApplicationError> {
        match self
            .query_application
            .get_context(GetContextQuery {
                root_node_id: about.to_string(),
                role: "memory".to_string(),
                depth: MAX_NATIVE_GRAPH_TRAVERSAL_DEPTH,
                requested_scopes: Vec::new(),
                render_options: ContextRenderOptions::default(),
            })
            .await
        {
            Ok(result) => Ok(existing_refs_from_bundle(&result.bundle)),
            Err(ApplicationError::NotFound(_)) => Ok(ExistingMemoryRefs::default()),
            Err(error) => Err(error),
        }
    }
}

fn memory_render_options(
    token_budget: u32,
    max_tier: Option<ResolutionTier>,
    rehydration_mode: RehydrationMode,
    endpoint_hint: EndpointHint,
) -> ContextRenderOptions {
    ContextRenderOptions {
        focus_node_id: None,
        token_budget: (token_budget > 0).then_some(token_budget),
        max_tier,
        rehydration_mode,
        endpoint_hint,
    }
}

fn requested_dimension_scopes(selection: &DimensionSelection) -> Vec<String> {
    match selection.mode() {
        DimensionSelectionMode::Only => selection.dimensions().iter().cloned().collect(),
        DimensionSelectionMode::Except | DimensionSelectionMode::All => Vec::new(),
    }
}

fn apply_dimension_selection(
    mut result: GetContextResult,
    dimensions: &DimensionSelection,
    render_options: &ContextRenderOptions,
) -> Result<GetContextResult, ApplicationError> {
    if dimensions.mode() == DimensionSelectionMode::All {
        return Ok(result);
    }

    result.bundle = filter_bundle_by_memory_dimensions(&result.bundle, dimensions)?;
    result.rendered = render_graph_bundle_with_options(&result.bundle, render_options);
    Ok(result)
}

fn filter_bundle_by_memory_dimensions(
    bundle: &RehydrationBundle,
    dimensions: &DimensionSelection,
) -> Result<RehydrationBundle, ApplicationError> {
    let mut included_node_ids = BTreeSet::from([bundle.root_node().node_id().to_string()]);
    let mut selected_entry_ids = BTreeSet::new();

    for relationship in bundle
        .relationships()
        .iter()
        .filter(|relationship| relationship.relationship_type() == "contains_entry")
    {
        let dimension = relationship.explanation().dimension().unwrap_or_default();
        if dimensions.includes(dimension) {
            included_node_ids.insert(relationship.source_node_id().to_string());
            included_node_ids.insert(relationship.target_node_id().to_string());
            selected_entry_ids.insert(relationship.target_node_id().to_string());
        }
    }

    for relationship in bundle.relationships().iter().filter(|relationship| {
        relationship.relationship_type() == "supports"
            && selected_entry_ids.contains(relationship.target_node_id())
    }) {
        included_node_ids.insert(relationship.source_node_id().to_string());
    }

    let neighbor_nodes = bundle
        .neighbor_nodes()
        .iter()
        .filter(|node| included_node_ids.contains(node.node_id()))
        .cloned()
        .collect::<Vec<_>>();
    let relationships = bundle
        .relationships()
        .iter()
        .filter(|relationship| {
            if relationship.relationship_type() == "contains_entry" {
                let dimension = relationship.explanation().dimension().unwrap_or_default();
                return dimensions.includes(dimension);
            }
            included_node_ids.contains(relationship.source_node_id())
                && included_node_ids.contains(relationship.target_node_id())
        })
        .cloned()
        .collect::<Vec<_>>();
    let node_details = bundle
        .node_details()
        .iter()
        .filter(|detail| included_node_ids.contains(detail.node_id()))
        .cloned()
        .collect::<Vec<_>>();

    RehydrationBundle::new(
        bundle.root_node_id().clone(),
        bundle.role().clone(),
        bundle.root_node().clone(),
        neighbor_nodes,
        relationships,
        node_details,
        bundle.metadata().clone(),
    )
    .map_err(Into::into)
}

fn existing_refs_from_bundle(bundle: &RehydrationBundle) -> ExistingMemoryRefs {
    let mut refs = BTreeSet::from([bundle.root_node().node_id().to_string()]);
    let mut dimensions = BTreeSet::new();

    for node in bundle.neighbor_nodes() {
        refs.insert(node.node_id().to_string());
        if node.node_kind() == "memory_dimension" {
            dimensions.insert(node.node_id().to_string());
        }
    }

    for relationship in bundle
        .relationships()
        .iter()
        .filter(|relationship| relationship.relationship_type() == "contains_entry")
    {
        dimensions.insert(relationship.source_node_id().to_string());
    }

    ExistingMemoryRefs { refs, dimensions }
}

fn is_unknown_memory_ref_validation(error: &ApplicationError) -> bool {
    matches!(
        error,
        ApplicationError::Validation(message)
            if message.contains("unknown ref") || message.contains("unknown dimension scope")
    )
}
