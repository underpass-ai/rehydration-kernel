use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::{DomainError, TemporalCoordinate, TemporalCursor, TemporalDirection};

use super::TemporalTraversalRequest;
use super::axis_key::{TemporalAxisKey, primary_coordinate_key};
use super::position::{ResolvedTemporalCursor, TemporalPosition};

pub(super) fn resolve_cursor(
    positions: &[TemporalPosition],
    cursor: &TemporalCursor,
) -> Result<ResolvedTemporalCursor, DomainError> {
    match cursor {
        TemporalCursor::Ref(ref_id) => positions
            .iter()
            .filter(|position| position.ref_id == *ref_id)
            .min()
            .map(|position| ResolvedTemporalCursor {
                axis_key: position.axis_key.clone(),
                coordinate: position.coordinate.clone(),
            })
            .ok_or_else(|| {
                DomainError::InvalidState(format!("temporal cursor ref not found: {ref_id}"))
            }),
        TemporalCursor::Time(value) => Ok(ResolvedTemporalCursor {
            axis_key: TemporalAxisKey::time(value),
            coordinate: TemporalCoordinate::cursor_time(value.clone())?,
        }),
        TemporalCursor::Sequence(value) => Ok(ResolvedTemporalCursor {
            axis_key: TemporalAxisKey::sequence(*value),
            coordinate: TemporalCoordinate::cursor_sequence(*value)?,
        }),
    }
}

pub(super) fn select_positions(
    positions: &[TemporalPosition],
    cursor: &ResolvedTemporalCursor,
    request: &TemporalTraversalRequest,
) -> Vec<TemporalPosition> {
    let comparable = positions
        .iter()
        .filter(|position| position.axis_key.axis() == cursor.axis_key.axis())
        .cloned()
        .collect::<Vec<_>>();

    match request.direction() {
        TemporalDirection::Goto => take_last(
            comparable
                .into_iter()
                .filter(|position| position.axis_key <= cursor.axis_key)
                .collect(),
            request.limit_entries().unwrap_or(1),
        ),
        TemporalDirection::Rewind => take_last(
            comparable
                .into_iter()
                .filter(|position| position.axis_key < cursor.axis_key)
                .collect(),
            request.limit_entries().unwrap_or(5),
        ),
        TemporalDirection::Forward => comparable
            .into_iter()
            .filter(|position| position.axis_key > cursor.axis_key)
            .take(request.limit_entries().unwrap_or(5))
            .collect(),
        TemporalDirection::Near => {
            let before = take_last(
                comparable
                    .iter()
                    .filter(|position| position.axis_key < cursor.axis_key)
                    .cloned()
                    .collect(),
                request.window().before_entries(),
            );
            let after = comparable
                .into_iter()
                .filter(|position| position.axis_key > cursor.axis_key)
                .take(request.window().after_entries());

            before.into_iter().chain(after).collect()
        }
    }
}

pub(super) fn ordered_unique_ref_ids(mut selected_positions: Vec<TemporalPosition>) -> Vec<String> {
    selected_positions.sort();
    let mut seen = BTreeSet::new();
    selected_positions
        .into_iter()
        .filter_map(|position| {
            if seen.insert(position.ref_id.clone()) {
                Some(position.ref_id)
            } else {
                None
            }
        })
        .collect()
}

pub(super) fn coordinates_by_ref(
    positions: &[TemporalPosition],
) -> BTreeMap<String, Vec<TemporalCoordinate>> {
    let mut coordinates = BTreeMap::<String, Vec<TemporalCoordinate>>::new();
    for position in positions {
        let entry = coordinates.entry(position.ref_id.clone()).or_default();
        if !entry.contains(&position.coordinate) {
            entry.push(position.coordinate.clone());
        }
    }

    for coordinates in coordinates.values_mut() {
        coordinates.sort_by(compare_coordinates);
    }

    coordinates
}

fn take_last(mut positions: Vec<TemporalPosition>, limit: usize) -> Vec<TemporalPosition> {
    positions.sort();
    let keep_from = positions.len().saturating_sub(limit);
    positions.into_iter().skip(keep_from).collect()
}

fn compare_coordinates(left: &TemporalCoordinate, right: &TemporalCoordinate) -> Ordering {
    primary_coordinate_key(left)
        .cmp(&primary_coordinate_key(right))
        .then_with(|| left.dimension().cmp(right.dimension()))
        .then_with(|| left.scope_id().cmp(right.scope_id()))
}
