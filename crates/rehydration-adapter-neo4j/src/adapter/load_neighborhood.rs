use std::collections::{BTreeMap, BTreeSet};

use neo4rs::Graph;
use rehydration_ports::{
    GraphNeighborhoodReader, NodeNeighborhood, NodeProjection, NodeRelationProjection, PortError,
};

use super::projection_store::Neo4jProjectionStore;
use super::queries::{load_neighborhood_query, load_root_node_query};
use super::row_mapping::{node_projection_from_row, row_string};

impl Neo4jProjectionStore {
    async fn load_root_node(
        &self,
        graph: &Graph,
        root_node_id: &str,
    ) -> Result<Option<NodeProjection>, PortError> {
        let Some(row) = self
            .fetch_optional_row(
                graph,
                load_root_node_query(root_node_id),
                &format!("load root node for `{root_node_id}`"),
            )
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
    ) -> Result<Vec<neo4rs::Row>, PortError> {
        self.fetch_rows(
            graph,
            load_neighborhood_query(root_node_id),
            &format!("load node neighborhood for `{root_node_id}`"),
        )
        .await
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
