use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use neo4rs::{Graph, Row, query};
use rehydration_ports::{
    GraphNeighborhoodReader, NodeNeighborhood, NodeProjection, NodeRelationProjection, PortError,
    ProjectionMutation, ProjectionWriter,
};
use tokio::sync::OnceCell;

use super::cypher::node_scoped_query;
use super::endpoint::Neo4jEndpoint;
use super::queries::{NODE_NEIGHBORHOOD_QUERY, ROOT_NODE_QUERY};
use super::row_mapping::{node_projection_from_row, row_string, serialize_properties};

#[derive(Clone)]
pub struct Neo4jProjectionStore {
    endpoint: Neo4jEndpoint,
    graph: Arc<OnceCell<Arc<Graph>>>,
}

pub type Neo4jProjectionReader = Neo4jProjectionStore;

impl fmt::Debug for Neo4jProjectionStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Neo4jProjectionStore")
            .field("endpoint", &self.endpoint)
            .field("connected", &self.graph.get().is_some())
            .finish()
    }
}

impl Neo4jProjectionStore {
    pub fn new(graph_uri: impl Into<String>) -> Result<Self, PortError> {
        let endpoint = Neo4jEndpoint::parse(graph_uri.into())?;
        Ok(Self {
            endpoint,
            graph: Arc::new(OnceCell::new()),
        })
    }

    async fn graph(&self) -> Result<Arc<Graph>, PortError> {
        let graph = self
            .graph
            .get_or_try_init(|| async {
                Graph::new(
                    &self.endpoint.connection_uri,
                    &self.endpoint.user,
                    &self.endpoint.password,
                )
                .await
                .map(Arc::new)
                .map_err(|error| {
                    PortError::Unavailable(format!(
                        "neo4j connection failed for `{}`: {error}",
                        self.endpoint.connection_uri
                    ))
                })
            })
            .await?;

        Ok(Arc::clone(graph))
    }

    async fn apply_node_projection(
        &self,
        graph: &Graph,
        node: &NodeProjection,
    ) -> Result<(), PortError> {
        graph
            .run(
                query(
                    "
MERGE (node:ProjectionNode {node_id: $node_id})
SET node.node_kind = $node_kind,
    node.title = $title,
    node.summary = $summary,
    node.status = $status,
    node.node_labels = $node_labels,
    node.properties_json = $properties_json
                    ",
                )
                .param("node_id", node.node_id.as_str())
                .param("node_kind", node.node_kind.as_str())
                .param("title", node.title.as_str())
                .param("summary", node.summary.as_str())
                .param("status", node.status.as_str())
                .param("node_labels", node.labels.clone())
                .param("properties_json", serialize_properties(&node.properties)?),
            )
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j apply node projection failed for node `{}`: {error}",
                    node.node_id
                ))
            })
    }

    async fn apply_relation_projection(
        &self,
        graph: &Graph,
        relation: &NodeRelationProjection,
    ) -> Result<(), PortError> {
        graph
            .run(
                query(
                    "
MERGE (source:ProjectionNode {node_id: $source_node_id})
ON CREATE SET source.node_kind = 'unknown',
              source.title = '',
              source.summary = '',
              source.status = 'STATUS_UNSPECIFIED',
              source.node_labels = [],
              source.properties_json = '{}'
MERGE (target:ProjectionNode {node_id: $target_node_id})
ON CREATE SET target.node_kind = 'unknown',
              target.title = '',
              target.summary = '',
              target.status = 'STATUS_UNSPECIFIED',
              target.node_labels = [],
              target.properties_json = '{}'
MERGE (source)-[edge:RELATED_TO {relation_type: $relation_type}]->(target)
                    ",
                )
                .param("source_node_id", relation.source_node_id.as_str())
                .param("target_node_id", relation.target_node_id.as_str())
                .param("relation_type", relation.relation_type.as_str()),
            )
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j apply relation projection failed for edge `{} -> {}`: {error}",
                    relation.source_node_id, relation.target_node_id
                ))
            })
    }

    async fn load_root_node(
        &self,
        graph: &Graph,
        root_node_id: &str,
    ) -> Result<Option<NodeProjection>, PortError> {
        let Some(row) = self
            .fetch_optional_row_for_node(graph, ROOT_NODE_QUERY, root_node_id, "load root node")
            .await?
        else {
            return Ok(None);
        };

        Ok(Some(node_projection_from_row(&row, "", "root node")?))
    }

    async fn load_neighbor_rows(
        &self,
        graph: &Graph,
        root_node_id: &str,
    ) -> Result<Vec<Row>, PortError> {
        self.fetch_rows_for_node(
            graph,
            NODE_NEIGHBORHOOD_QUERY,
            root_node_id,
            "load node neighborhood",
        )
        .await
    }

    async fn fetch_optional_row_for_node(
        &self,
        graph: &Graph,
        statement: &str,
        root_node_id: &str,
        operation: &str,
    ) -> Result<Option<Row>, PortError> {
        let mut rows = graph
            .execute(node_scoped_query(statement, root_node_id))
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j {operation} failed for root node `{root_node_id}`: {error}"
                ))
            })?;

        rows.next().await.map_err(|error| {
            PortError::Unavailable(format!(
                "neo4j {operation} stream failed for root node `{root_node_id}`: {error}"
            ))
        })
    }

    async fn fetch_rows_for_node(
        &self,
        graph: &Graph,
        statement: &str,
        root_node_id: &str,
        operation: &str,
    ) -> Result<Vec<Row>, PortError> {
        let mut rows = graph
            .execute(node_scoped_query(statement, root_node_id))
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j {operation} failed for root node `{root_node_id}`: {error}"
                ))
            })?;

        let mut collected = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| {
            PortError::Unavailable(format!(
                "neo4j {operation} stream failed for root node `{root_node_id}`: {error}"
            ))
        })? {
            collected.push(row);
        }

        Ok(collected)
    }
}

impl GraphNeighborhoodReader for Neo4jProjectionStore {
    async fn load_neighborhood(
        &self,
        root_node_id: &str,
    ) -> Result<Option<NodeNeighborhood>, PortError> {
        let graph = self.graph().await?;
        let Some(root) = self.load_root_node(&graph, root_node_id).await? else {
            return Ok(None);
        };
        let rows = self.load_neighbor_rows(&graph, root_node_id).await?;

        let mut neighbors_by_id = BTreeMap::<String, NodeProjection>::new();
        let mut relation_keys = BTreeSet::<(String, String, String)>::new();
        let mut relations = Vec::new();

        for row in rows {
            let neighbor_node_id = row_string(&row, "neighbor_node_id", "neighbor node")?;
            let relation_type = row_string(&row, "relation_type", "neighbor relation")?;
            if neighbor_node_id.is_empty() || relation_type.is_empty() {
                continue;
            }

            neighbors_by_id
                .entry(neighbor_node_id.clone())
                .or_insert(node_projection_from_row(
                    &row,
                    "neighbor_",
                    "neighbor node",
                )?);
            let source_node_id = row_string(&row, "source_node_id", "neighbor relation")?;
            let target_node_id = row_string(&row, "target_node_id", "neighbor relation")?;
            let relation_key = (
                source_node_id.clone(),
                target_node_id.clone(),
                relation_type.clone(),
            );

            if relation_keys.insert(relation_key) {
                relations.push(NodeRelationProjection {
                    source_node_id,
                    target_node_id,
                    relation_type,
                });
            }
        }

        Ok(Some(NodeNeighborhood {
            root,
            neighbors: neighbors_by_id.into_values().collect(),
            relations,
        }))
    }
}

impl ProjectionWriter for Neo4jProjectionStore {
    async fn apply_mutations(&self, mutations: Vec<ProjectionMutation>) -> Result<(), PortError> {
        let graph = self.graph().await?;

        for mutation in mutations {
            match mutation {
                ProjectionMutation::UpsertNode(node) => {
                    self.apply_node_projection(&graph, &node).await?;
                }
                ProjectionMutation::UpsertNodeRelation(relation) => {
                    self.apply_relation_projection(&graph, &relation).await?;
                }
                ProjectionMutation::UpsertNodeDetail(detail) => {
                    return Err(PortError::InvalidState(format!(
                        "neo4j graph projection writer does not persist node detail `{}`",
                        detail.node_id
                    )));
                }
            }
        }

        Ok(())
    }
}
