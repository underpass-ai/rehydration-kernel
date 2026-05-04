mod axis_key;
mod extract;
mod position;
mod select;

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DimensionSelection, DimensionSelectionMode, DomainError, RehydrationBundle, TemporalCoordinate,
    TemporalCursor, TemporalDirection, TemporalWindow,
};

use self::extract::{bundle_nodes_by_id, temporal_positions};
use self::position::TemporalPosition;
use self::select::{coordinates_by_ref, ordered_unique_ref_ids, resolve_cursor, select_positions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalTraversalRequest {
    direction: TemporalDirection,
    cursor: TemporalCursor,
    dimensions: DimensionSelection,
    window: TemporalWindow,
    limit_entries: Option<usize>,
}

impl TemporalTraversalRequest {
    pub fn new(direction: TemporalDirection, cursor: TemporalCursor) -> Self {
        Self {
            direction,
            cursor,
            dimensions: DimensionSelection::all(),
            window: TemporalWindow::default(),
            limit_entries: None,
        }
    }

    pub fn with_dimensions(mut self, dimensions: DimensionSelection) -> Self {
        self.dimensions = dimensions;
        self
    }

    pub fn with_window(mut self, window: TemporalWindow) -> Self {
        self.window = window;
        self
    }

    pub fn with_limit_entries(mut self, limit_entries: usize) -> Result<Self, DomainError> {
        if limit_entries == 0 {
            return Err(DomainError::InvalidState(
                "temporal limit_entries must be greater than zero".to_string(),
            ));
        }
        self.limit_entries = Some(limit_entries);
        Ok(self)
    }

    pub fn direction(&self) -> TemporalDirection {
        self.direction
    }

    pub fn cursor(&self) -> &TemporalCursor {
        &self.cursor
    }

    pub fn dimensions(&self) -> &DimensionSelection {
        &self.dimensions
    }

    pub(super) fn window(&self) -> TemporalWindow {
        self.window
    }

    pub(super) fn limit_entries(&self) -> Option<usize> {
        self.limit_entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalEntry {
    ref_id: String,
    kind: String,
    text: String,
    coordinates: Vec<TemporalCoordinate>,
}

impl TemporalEntry {
    pub fn ref_id(&self) -> &str {
        &self.ref_id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn coordinates(&self) -> &[TemporalCoordinate] {
        &self.coordinates
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalTraversalResult {
    direction: TemporalDirection,
    resolved_cursor: TemporalCoordinate,
    requested_dimensions: DimensionSelection,
    included_dimensions: Vec<String>,
    missing_dimensions: Vec<String>,
    entries: Vec<TemporalEntry>,
    missing: Vec<String>,
}

impl TemporalTraversalResult {
    pub fn direction(&self) -> TemporalDirection {
        self.direction
    }

    pub fn resolved_cursor(&self) -> &TemporalCoordinate {
        &self.resolved_cursor
    }

    pub fn requested_dimensions(&self) -> &DimensionSelection {
        &self.requested_dimensions
    }

    pub fn included_dimensions(&self) -> &[String] {
        &self.included_dimensions
    }

    pub fn missing_dimensions(&self) -> &[String] {
        &self.missing_dimensions
    }

    pub fn entries(&self) -> &[TemporalEntry] {
        &self.entries
    }

    pub fn missing(&self) -> &[String] {
        &self.missing
    }
}

pub struct TemporalMemoryTraversal;

impl TemporalMemoryTraversal {
    pub fn traverse(
        bundle: &RehydrationBundle,
        request: &TemporalTraversalRequest,
    ) -> Result<TemporalTraversalResult, DomainError> {
        let nodes = bundle_nodes_by_id(bundle);
        let mut positions = temporal_positions(bundle, &nodes)?
            .into_iter()
            .filter(|position| {
                request.dimensions.includes_coordinate(
                    position.coordinate.dimension(),
                    position.coordinate.scope_id(),
                )
            })
            .collect::<Vec<_>>();
        positions.sort();
        let available_dimensions = dimensions_from_positions(&positions);

        let cursor = resolve_cursor(&positions, request.cursor())?;
        let has_comparable_positions = positions
            .iter()
            .any(|position| position.axis_key.axis() == cursor.axis_key.axis());
        let selected_positions = select_positions(&positions, &cursor, request);
        let selected_ref_ids = ordered_unique_ref_ids(selected_positions);
        let coordinates_by_ref = coordinates_by_ref(&positions);
        let entries = build_entries(
            selected_ref_ids,
            &nodes,
            &coordinates_by_ref,
            request.dimensions(),
        );
        let included_dimensions = included_dimensions(&entries);
        let coverage_dimensions = if entries.is_empty() && has_comparable_positions {
            &available_dimensions
        } else {
            &included_dimensions
        };
        let missing_dimensions = missing_dimensions(request.dimensions(), coverage_dimensions);
        let missing = if positions.is_empty() || !has_comparable_positions {
            vec!["temporal_positions".to_string()]
        } else {
            Vec::new()
        };

        Ok(TemporalTraversalResult {
            direction: request.direction,
            resolved_cursor: cursor.coordinate,
            requested_dimensions: request.dimensions.clone(),
            included_dimensions,
            missing_dimensions,
            entries,
            missing,
        })
    }
}

fn build_entries(
    selected_ref_ids: Vec<String>,
    nodes: &BTreeMap<String, (String, String)>,
    coordinates_by_ref: &BTreeMap<String, Vec<TemporalCoordinate>>,
    dimensions: &DimensionSelection,
) -> Vec<TemporalEntry> {
    selected_ref_ids
        .into_iter()
        .filter_map(|ref_id| {
            let (kind, text) = nodes.get(&ref_id)?;
            let coordinates = coordinates_by_ref
                .get(&ref_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|coordinate| {
                    dimensions.includes_coordinate(coordinate.dimension(), coordinate.scope_id())
                })
                .collect::<Vec<_>>();

            Some(TemporalEntry {
                ref_id,
                kind: kind.clone(),
                text: text.clone(),
                coordinates,
            })
        })
        .collect()
}

fn included_dimensions(entries: &[TemporalEntry]) -> Vec<String> {
    entries
        .iter()
        .flat_map(|entry| entry.coordinates.iter())
        .map(|coordinate| coordinate.dimension().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn dimensions_from_positions(positions: &[TemporalPosition]) -> Vec<String> {
    positions
        .iter()
        .map(|position| position.coordinate.dimension().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn missing_dimensions(requested: &DimensionSelection, included: &[String]) -> Vec<String> {
    if requested.mode() != DimensionSelectionMode::Only {
        return Vec::new();
    }

    let included = included.iter().cloned().collect::<BTreeSet<_>>();
    requested
        .dimensions()
        .difference(&included)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        BundleMetadata, BundleNode, BundleRelationship, CaseId, RelationExplanation,
        RelationSemanticClass, Role,
    };

    use super::*;

    #[test]
    fn near_includes_exact_cursor_positions_between_before_and_after() {
        let bundle = temporal_bundle(&[
            ("claim:one", "conversation", "conversation:main", 1),
            ("claim:two", "conversation", "conversation:main", 2),
            ("claim:three", "conversation", "conversation:main", 3),
        ]);
        let request = TemporalTraversalRequest::new(
            TemporalDirection::Near,
            TemporalCursor::sequence(2).expect("sequence cursor should be valid"),
        )
        .with_dimensions(DimensionSelection::only(["conversation"]))
        .with_window(TemporalWindow::new(1, 1));

        let result =
            TemporalMemoryTraversal::traverse(&bundle, &request).expect("near should traverse");
        let refs = result
            .entries()
            .iter()
            .map(|entry| entry.ref_id())
            .collect::<Vec<_>>();

        assert_eq!(refs, vec!["claim:one", "claim:two", "claim:three"]);
        assert!(result.missing().is_empty());
    }

    #[test]
    fn near_includes_exact_position_when_no_neighbors_exist() {
        let bundle = temporal_bundle(&[("claim:one", "decision", "decision:main", 1)]);
        let request = TemporalTraversalRequest::new(
            TemporalDirection::Near,
            TemporalCursor::sequence(1).expect("sequence cursor should be valid"),
        )
        .with_dimensions(DimensionSelection::only(["decision"]))
        .with_window(TemporalWindow::new(2, 2));

        let result =
            TemporalMemoryTraversal::traverse(&bundle, &request).expect("near should traverse");

        assert_eq!(result.entries()[0].ref_id(), "claim:one");
        assert_eq!(result.included_dimensions(), &["decision".to_string()]);
        assert!(result.missing_dimensions().is_empty());
        assert!(result.missing().is_empty());
    }

    #[test]
    fn forward_boundary_reports_no_entries_without_missing_positions() {
        let bundle = temporal_bundle(&[("claim:one", "decision", "decision:main", 1)]);
        let request = TemporalTraversalRequest::new(
            TemporalDirection::Forward,
            TemporalCursor::sequence(1).expect("sequence cursor should be valid"),
        )
        .with_dimensions(DimensionSelection::only(["decision"]));

        let result =
            TemporalMemoryTraversal::traverse(&bundle, &request).expect("forward should traverse");

        assert!(result.entries().is_empty());
        assert!(result.missing_dimensions().is_empty());
        assert!(result.missing().is_empty());
    }

    fn temporal_bundle(entries: &[(&str, &str, &str, u32)]) -> RehydrationBundle {
        let mut nodes = BTreeMap::new();
        for (ref_id, _, scope_id, _) in entries {
            nodes.insert(
                (*scope_id).to_string(),
                node(scope_id, "memory_dimension", scope_id),
            );
            nodes.insert((*ref_id).to_string(), node(ref_id, "claim", ref_id));
        }

        RehydrationBundle::new(
            CaseId::new("question:a").expect("case id should be valid"),
            Role::new("temporal-reader").expect("role should be valid"),
            node("question:a", "question", "Question A"),
            nodes.into_values().collect(),
            entries
                .iter()
                .map(|(ref_id, dimension, scope_id, sequence)| {
                    BundleRelationship::new(
                        *scope_id,
                        *ref_id,
                        "contains_entry",
                        RelationExplanation::new(RelationSemanticClass::Structural)
                            .with_dimension(*dimension)
                            .with_scope_id(*scope_id)
                            .with_sequence(*sequence),
                    )
                })
                .collect(),
            Vec::new(),
            BundleMetadata::initial("test"),
        )
        .expect("test bundle should be valid")
    }

    fn node(node_id: &str, kind: &str, title: &str) -> BundleNode {
        BundleNode::new(
            node_id,
            kind,
            title,
            title,
            "ACTIVE",
            Vec::new(),
            BTreeMap::new(),
        )
    }
}
