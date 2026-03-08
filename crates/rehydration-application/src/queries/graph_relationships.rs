use std::collections::BTreeMap;

use rehydration_domain::{GraphNeighborhoodReader, NodeNeighborhood};

use crate::ApplicationError;
use crate::queries::AdminQueryApplicationService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetGraphRelationshipsQuery {
    pub node_id: String,
    pub node_kind: Option<String>,
    pub depth: u32,
    pub include_reverse_edges: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNodeView {
    pub node_id: String,
    pub node_kind: String,
    pub title: String,
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRelationshipView {
    pub source_node_id: String,
    pub target_node_id: String,
    pub relationship_type: String,
    pub properties: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetGraphRelationshipsResult {
    pub root: GraphNodeView,
    pub neighbors: Vec<GraphNodeView>,
    pub relationships: Vec<GraphRelationshipView>,
    pub observed_at: std::time::SystemTime,
}

#[derive(Debug)]
pub struct GetGraphRelationshipsUseCase<G> {
    graph_reader: G,
}

impl<G> GetGraphRelationshipsUseCase<G>
where
    G: GraphNeighborhoodReader + Send + Sync,
{
    pub fn new(graph_reader: G) -> Self {
        Self { graph_reader }
    }

    pub async fn execute(
        &self,
        query: GetGraphRelationshipsQuery,
    ) -> Result<GetGraphRelationshipsResult, ApplicationError> {
        let node_id = trim_to_option(&query.node_id)
            .ok_or_else(|| ApplicationError::Validation("node_id cannot be empty".to_string()))?;
        let neighborhood = self
            .graph_reader
            .load_neighborhood(&node_id)
            .await?
            .unwrap_or_else(|| placeholder_neighborhood(&node_id, query.node_kind));

        Ok(GetGraphRelationshipsResult {
            root: map_node(&neighborhood.root),
            neighbors: neighborhood.neighbors.iter().map(map_node).collect(),
            relationships: neighborhood
                .relations
                .iter()
                .map(|relation| GraphRelationshipView {
                    source_node_id: relation.source_node_id.clone(),
                    target_node_id: relation.target_node_id.clone(),
                    relationship_type: relation.relation_type.clone(),
                    properties: BTreeMap::new(),
                })
                .collect(),
            observed_at: std::time::SystemTime::now(),
        })
    }
}

impl<G, D> AdminQueryApplicationService<G, D>
where
    G: GraphNeighborhoodReader + Send + Sync,
{
    pub async fn get_graph_relationships(
        &self,
        query: GetGraphRelationshipsQuery,
    ) -> Result<GetGraphRelationshipsResult, ApplicationError> {
        GetGraphRelationshipsUseCase::new(std::sync::Arc::clone(&self.graph_reader))
            .execute(query)
            .await
    }
}

fn map_node(node: &rehydration_domain::NodeProjection) -> GraphNodeView {
    GraphNodeView {
        node_id: node.node_id.clone(),
        node_kind: node.node_kind.clone(),
        title: node.title.clone(),
        labels: node.labels.clone(),
        properties: node.properties.clone(),
    }
}

fn placeholder_neighborhood(node_id: &str, node_kind: Option<String>) -> NodeNeighborhood {
    NodeNeighborhood {
        root: rehydration_domain::NodeProjection {
            node_id: node_id.to_string(),
            node_kind: node_kind.unwrap_or_else(|| "unknown".to_string()),
            title: format!("Node {node_id}"),
            summary: String::new(),
            status: "UNKNOWN".to_string(),
            labels: Vec::new(),
            properties: BTreeMap::new(),
        },
        neighbors: Vec::new(),
        relations: Vec::new(),
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
