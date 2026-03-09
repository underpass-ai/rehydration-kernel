use rehydration_application::GetGraphRelationshipsQuery;
use rehydration_proto::fleet_context_v1::GetGraphRelationshipsRequest;

pub(crate) fn map_get_graph_relationships_query(
    request: GetGraphRelationshipsRequest,
) -> GetGraphRelationshipsQuery {
    GetGraphRelationshipsQuery {
        node_id: request.node_id,
        node_kind: trim_to_option(request.node_type),
        depth: clamp_depth(request.depth),
        include_reverse_edges: false,
    }
}

fn trim_to_option(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn clamp_depth(value: i32) -> u32 {
    value.clamp(1, 3) as u32
}

#[cfg(test)]
mod tests {
    use rehydration_proto::fleet_context_v1::GetGraphRelationshipsRequest;

    use super::map_get_graph_relationships_query;

    #[test]
    fn depth_is_clamped_to_external_contract_bounds() {
        let low = map_get_graph_relationships_query(GetGraphRelationshipsRequest {
            node_id: "node-1".to_string(),
            node_type: String::new(),
            depth: 0,
        });
        let high = map_get_graph_relationships_query(GetGraphRelationshipsRequest {
            node_id: "node-1".to_string(),
            node_type: "Story".to_string(),
            depth: 8,
        });

        assert_eq!(low.depth, 1);
        assert_eq!(high.depth, 3);
        assert_eq!(high.node_kind.as_deref(), Some("Story"));
    }
}
