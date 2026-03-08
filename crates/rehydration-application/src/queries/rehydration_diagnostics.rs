use rehydration_domain::{BundleMetadata, GraphNeighborhoodReader, NodeDetailReader};

use crate::ApplicationError;
use crate::queries::{
    AdminQueryApplicationService, BundleAssembler, NodeCentricProjectionReader, render_graph_bundle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRehydrationDiagnosticsQuery {
    pub root_node_id: String,
    pub roles: Vec<String>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RehydrationDiagnosticView {
    pub role: String,
    pub version: BundleMetadata,
    pub selected_nodes: u32,
    pub selected_relationships: u32,
    pub detailed_nodes: u32,
    pub estimated_tokens: u32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetRehydrationDiagnosticsResult {
    pub diagnostics: Vec<RehydrationDiagnosticView>,
    pub observed_at: std::time::SystemTime,
}

#[derive(Debug)]
pub struct GetRehydrationDiagnosticsUseCase<G, D> {
    graph_reader: G,
    detail_reader: D,
    generator_version: &'static str,
}

impl<G, D> GetRehydrationDiagnosticsUseCase<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
{
    pub fn new(graph_reader: G, detail_reader: D, generator_version: &'static str) -> Self {
        Self {
            graph_reader,
            detail_reader,
            generator_version,
        }
    }

    pub async fn execute(
        &self,
        query: GetRehydrationDiagnosticsQuery,
    ) -> Result<GetRehydrationDiagnosticsResult, ApplicationError> {
        if query.roles.is_empty() {
            return Err(ApplicationError::Validation(
                "roles cannot be empty".to_string(),
            ));
        }

        let phase = query
            .phase
            .and_then(|value| trim_to_option(&value))
            .unwrap_or_else(|| "PHASE_UNSPECIFIED".to_string());
        let observed_at = std::time::SystemTime::now();
        let mut diagnostics = Vec::with_capacity(query.roles.len());
        let bundle_reader =
            NodeCentricProjectionReader::new(&self.graph_reader, &self.detail_reader);

        for role in &query.roles {
            let bundle = match bundle_reader
                .load_bundle(&query.root_node_id, role, self.generator_version)
                .await?
            {
                Some(bundle) => bundle,
                None => {
                    BundleAssembler::placeholder(&query.root_node_id, role, self.generator_version)?
                }
            };
            let rendered = render_graph_bundle(&bundle);
            diagnostics.push(RehydrationDiagnosticView {
                role: role.clone(),
                version: bundle.metadata().clone(),
                selected_nodes: bundle.stats().selected_nodes(),
                selected_relationships: bundle.stats().selected_relationships(),
                detailed_nodes: bundle.stats().detailed_nodes(),
                estimated_tokens: rendered.token_count,
                notes: vec![format!("phase={phase}")],
            });
        }

        Ok(GetRehydrationDiagnosticsResult {
            diagnostics,
            observed_at,
        })
    }
}

impl<G, D> AdminQueryApplicationService<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
    D: NodeDetailReader + Send + Sync,
{
    pub async fn get_rehydration_diagnostics(
        &self,
        query: GetRehydrationDiagnosticsQuery,
    ) -> Result<GetRehydrationDiagnosticsResult, ApplicationError> {
        GetRehydrationDiagnosticsUseCase::new(
            std::sync::Arc::clone(&self.graph_reader),
            std::sync::Arc::clone(&self.detail_reader),
            self.generator_version,
        )
        .execute(query)
        .await
    }
}

fn trim_to_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
