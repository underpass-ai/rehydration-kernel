use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use neo4rs::{Graph, Row, query};
use rehydration_domain::{
    CaseHeader, CaseId, Decision, DecisionRelation, Milestone, PlanHeader, Role, RoleContextPack,
    TaskImpact, WorkItem,
};
use rehydration_ports::{
    GraphNeighborhoodReader, NodeNeighborhood, NodeProjection, NodeRelationProjection, PortError,
    ProjectionMutation, ProjectionReader, ProjectionWriter,
};
use tokio::sync::OnceCell;

use super::cypher::{node_scoped_query, scoped_query};
use super::endpoint::Neo4jEndpoint;
use super::queries::{
    CASE_HEADER_QUERY, DECISION_RELATIONS_QUERY, DECISIONS_QUERY, IMPACTS_QUERY, MILESTONES_QUERY,
    NODE_NEIGHBORHOOD_QUERY, PLAN_HEADER_QUERY, ROOT_NODE_QUERY, ROOT_QUERY, WORK_ITEMS_QUERY,
};
use super::row_mapping::{
    ProjectionRootRecord, RawCaseHeaderRecord, RawDecisionRecord, RawMilestoneRecord,
    RawPlanHeaderRecord, RawProjectionRootRecord, RawWorkItemRecord, node_projection_from_row,
    row_i64, row_string, row_string_vec, serialize_properties, task_impact_from_row,
};

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

    async fn load_root(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<ProjectionRootRecord>, PortError> {
        let Some(row) = self
            .fetch_optional_row(graph, ROOT_QUERY, case_id, role, "load projection root")
            .await?
        else {
            return Ok(None);
        };

        let raw = RawProjectionRootRecord {
            latest_summary: row_string(&row, "latest_summary", "projection root")?,
            token_budget_hint: row_i64(&row, "token_budget_hint", "projection root")?,
        };

        Ok(Some(raw.try_into()?))
    }

    async fn load_case_header(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<CaseHeader, PortError> {
        let row = self
            .fetch_optional_row(graph, CASE_HEADER_QUERY, case_id, role, "load case header")
            .await?
            .ok_or_else(|| {
                PortError::InvalidState(format!(
                    "neo4j projection for case `{}` role `{}` is missing a case header",
                    case_id.as_str(),
                    role.as_str()
                ))
            })?;

        RawCaseHeaderRecord {
            case_id: row_string(&row, "case_id", "case header")?,
            title: row_string(&row, "title", "case header")?,
            summary: row_string(&row, "summary", "case header")?,
            status: row_string(&row, "status", "case header")?,
            created_at_millis: row_i64(&row, "created_at", "case header")?,
            created_by: row_string(&row, "created_by", "case header")?,
        }
        .try_into()
    }

    async fn load_plan_header(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<PlanHeader>, PortError> {
        let Some(row) = self
            .fetch_optional_row(graph, PLAN_HEADER_QUERY, case_id, role, "load plan header")
            .await?
        else {
            return Ok(None);
        };

        let raw = RawPlanHeaderRecord {
            plan_id: row_string(&row, "plan_id", "plan header")?,
            revision: row_i64(&row, "revision", "plan header")?,
            status: row_string(&row, "status", "plan header")?,
            work_items_total: row_i64(&row, "work_items_total", "plan header")?,
            work_items_completed: row_i64(&row, "work_items_completed", "plan header")?,
        };

        Ok(Some(raw.try_into()?))
    }

    async fn load_work_items(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<WorkItem>, PortError> {
        let rows = self
            .fetch_rows(graph, WORK_ITEMS_QUERY, case_id, role, "load work items")
            .await?;

        rows.into_iter()
            .map(|row| {
                RawWorkItemRecord {
                    work_item_id: row_string(&row, "work_item_id", "work item")?,
                    title: row_string(&row, "title", "work item")?,
                    summary: row_string(&row, "summary", "work item")?,
                    role: row_string(&row, "item_role", "work item")?,
                    phase: row_string(&row, "phase", "work item")?,
                    status: row_string(&row, "status", "work item")?,
                    dependency_ids: row_string_vec(&row, "dependency_ids", "work item")?,
                    priority: row_i64(&row, "priority", "work item")?,
                }
                .try_into()
            })
            .collect()
    }

    async fn load_decisions(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<Decision>, PortError> {
        let rows = self
            .fetch_rows(graph, DECISIONS_QUERY, case_id, role, "load decisions")
            .await?;

        rows.into_iter()
            .map(|row| {
                RawDecisionRecord {
                    decision_id: row_string(&row, "decision_id", "decision")?,
                    title: row_string(&row, "title", "decision")?,
                    rationale: row_string(&row, "rationale", "decision")?,
                    status: row_string(&row, "status", "decision")?,
                    owner: row_string(&row, "owner", "decision")?,
                    decided_at_millis: row_i64(&row, "decided_at", "decision")?,
                }
                .try_into()
            })
            .collect()
    }

    async fn load_decision_relations(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<DecisionRelation>, PortError> {
        let rows = self
            .fetch_rows(
                graph,
                DECISION_RELATIONS_QUERY,
                case_id,
                role,
                "load decision relations",
            )
            .await?;

        rows.into_iter()
            .map(|row| {
                Ok(DecisionRelation::new(
                    row_string(&row, "source_decision_id", "decision relation")?,
                    row_string(&row, "target_decision_id", "decision relation")?,
                    row_string(&row, "relation_type", "decision relation")?,
                ))
            })
            .collect()
    }

    async fn load_impacts(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<TaskImpact>, PortError> {
        let rows = self
            .fetch_rows(graph, IMPACTS_QUERY, case_id, role, "load task impacts")
            .await?;

        rows.into_iter()
            .map(|row| task_impact_from_row(&row))
            .collect()
    }

    async fn load_milestones(
        &self,
        graph: &Graph,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Vec<Milestone>, PortError> {
        let rows = self
            .fetch_rows(graph, MILESTONES_QUERY, case_id, role, "load milestones")
            .await?;

        rows.into_iter()
            .map(|row| {
                RawMilestoneRecord {
                    milestone_type: row_string(&row, "milestone_type", "milestone")?,
                    description: row_string(&row, "description", "milestone")?,
                    occurred_at_millis: row_i64(&row, "occurred_at", "milestone")?,
                    actor: row_string(&row, "actor", "milestone")?,
                }
                .try_into()
            })
            .collect()
    }

    async fn fetch_optional_row(
        &self,
        graph: &Graph,
        statement: &str,
        case_id: &CaseId,
        role: &Role,
        operation: &str,
    ) -> Result<Option<Row>, PortError> {
        let mut rows = graph
            .execute(scoped_query(statement, case_id, role))
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j {operation} failed for case `{}` role `{}`: {error}",
                    case_id.as_str(),
                    role.as_str()
                ))
            })?;

        rows.next().await.map_err(|error| {
            PortError::Unavailable(format!(
                "neo4j {operation} stream failed for case `{}` role `{}`: {error}",
                case_id.as_str(),
                role.as_str()
            ))
        })
    }

    async fn fetch_rows(
        &self,
        graph: &Graph,
        statement: &str,
        case_id: &CaseId,
        role: &Role,
        operation: &str,
    ) -> Result<Vec<Row>, PortError> {
        let mut rows = graph
            .execute(scoped_query(statement, case_id, role))
            .await
            .map_err(|error| {
                PortError::Unavailable(format!(
                    "neo4j {operation} failed for case `{}` role `{}`: {error}",
                    case_id.as_str(),
                    role.as_str()
                ))
            })?;

        let mut collected = Vec::new();
        while let Some(row) = rows.next().await.map_err(|error| {
            PortError::Unavailable(format!(
                "neo4j {operation} stream failed for case `{}` role `{}`: {error}",
                case_id.as_str(),
                role.as_str()
            ))
        })? {
            collected.push(row);
        }

        Ok(collected)
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
            relations.push(NodeRelationProjection {
                source_node_id: row_string(&row, "source_node_id", "neighbor relation")?,
                target_node_id: row_string(&row, "target_node_id", "neighbor relation")?,
                relation_type,
            });
        }

        Ok(Some(NodeNeighborhood {
            root,
            neighbors: neighbors_by_id.into_values().collect(),
            relations,
        }))
    }
}

impl ProjectionReader for Neo4jProjectionStore {
    async fn load_pack(
        &self,
        case_id: &CaseId,
        role: &Role,
    ) -> Result<Option<RoleContextPack>, PortError> {
        let graph = self.graph().await?;
        let Some(root) = self.load_root(&graph, case_id, role).await? else {
            return Ok(None);
        };

        let case_header = self.load_case_header(&graph, case_id, role).await?;
        let plan_header = self.load_plan_header(&graph, case_id, role).await?;
        let work_items = self.load_work_items(&graph, case_id, role).await?;
        let decisions = self.load_decisions(&graph, case_id, role).await?;
        let decision_relations = self.load_decision_relations(&graph, case_id, role).await?;
        let impacts = self.load_impacts(&graph, case_id, role).await?;
        let milestones = self.load_milestones(&graph, case_id, role).await?;

        Ok(Some(RoleContextPack::new(
            role.clone(),
            case_header,
            plan_header,
            work_items,
            decisions,
            decision_relations,
            impacts,
            milestones,
            root.latest_summary,
            root.token_budget_hint,
        )))
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
