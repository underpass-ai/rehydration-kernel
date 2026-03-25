use std::collections::BTreeMap;

use rehydration_domain::{BundleNode, BundleNodeDetail, BundleRelationship, RehydrationBundle};

use crate::queries::ContextRenderOptions;

/// Builds ordered sections from a bundle with salience-based prioritization.
///
/// Section order: root > focus node > explanatory relations > neighbor nodes > details.
/// This ensures that under token pressure, explanatory relationships survive
/// before neighbor nodes and details.
pub(crate) fn ordered_sections(
    bundle: &RehydrationBundle,
    detail_by_node_id: &BTreeMap<&str, &BundleNodeDetail>,
    options: &ContextRenderOptions,
) -> Vec<String> {
    let mut sections = Vec::new();
    sections.push(render_node(bundle.root_node()));

    let focus_node_id = focus_node_id(bundle, options);

    if let Some(focus_node_id) = focus_node_id
        && focus_node_id != bundle.root_node().node_id()
        && let Some(node) = bundle
            .neighbor_nodes()
            .iter()
            .find(|node| node.node_id() == focus_node_id)
    {
        sections.push(render_node(node));
    }

    for relationship in prioritized_relationships(bundle, focus_node_id) {
        sections.push(render_relationship(relationship));
    }

    for node in bundle.neighbor_nodes() {
        if Some(node.node_id()) != focus_node_id {
            sections.push(render_node(node));
        }
    }

    for detail in prioritized_details(bundle, focus_node_id) {
        sections.push(render_detail(detail, detail_by_node_id));
    }

    sections
}

fn focus_node_id<'a>(
    bundle: &'a RehydrationBundle,
    options: &'a ContextRenderOptions,
) -> Option<&'a str> {
    let focus_node_id = options.focus_node_id.as_deref()?;
    if bundle.root_node().node_id() == focus_node_id
        || bundle
            .neighbor_nodes()
            .iter()
            .any(|node| node.node_id() == focus_node_id)
    {
        Some(focus_node_id)
    } else {
        None
    }
}

fn prioritized_relationships<'a>(
    bundle: &'a RehydrationBundle,
    focus_node_id: Option<&'a str>,
) -> Vec<&'a BundleRelationship> {
    let mut relationships: Vec<_> = bundle.relationships().iter().collect();

    // Sort by semantic salience: causal/motivational before structural
    relationships.sort_by_key(|r| r.explanation().semantic_class().salience_rank());

    let Some(focus_node_id) = focus_node_id else {
        return relationships;
    };

    // Within each salience tier, focused relationships come first
    let (focused, remaining): (Vec<_>, Vec<_>) =
        relationships.into_iter().partition(|relationship| {
            relationship.source_node_id() == focus_node_id
                || relationship.target_node_id() == focus_node_id
        });

    focused.into_iter().chain(remaining).collect()
}

fn prioritized_details<'a>(
    bundle: &'a RehydrationBundle,
    focus_node_id: Option<&'a str>,
) -> Vec<&'a BundleNodeDetail> {
    let Some(focus_node_id) = focus_node_id else {
        return bundle.node_details().iter().collect();
    };

    let (focused, remaining): (Vec<_>, Vec<_>) = bundle
        .node_details()
        .iter()
        .partition(|detail| detail.node_id() == focus_node_id);

    focused.into_iter().chain(remaining).collect()
}

pub(crate) fn render_node(node: &BundleNode) -> String {
    let mut section = format!("Node {} ({})", node.title(), node.node_kind());
    if !node.summary().trim().is_empty() {
        section.push_str(": ");
        section.push_str(node.summary().trim());
    }
    if let Some(provenance) = node.provenance() {
        section.push_str(" [source:");
        section.push_str(provenance.source_kind().as_str());
        if let Some(agent) = provenance.source_agent() {
            section.push_str(" agent=");
            section.push_str(agent);
        }
        if let Some(observed) = provenance.observed_at() {
            section.push_str(" observed=");
            section.push_str(observed);
        }
        section.push(']');
    }
    section
}

pub(crate) fn render_relationship(relationship: &BundleRelationship) -> String {
    let mut section = format!(
        "Relationship {} --{}--> {}",
        relationship.source_node_id(),
        relationship.relationship_type(),
        relationship.target_node_id()
    );

    section.push_str(" [");
    section.push_str(relationship.explanation().semantic_class().as_str());
    section.push(']');

    if let Some(rationale) = relationship
        .explanation()
        .rationale()
        .or(relationship.explanation().motivation())
    {
        section.push_str(" because ");
        section.push_str(rationale);
    }
    if let Some(method) = relationship.explanation().method() {
        section.push_str(" via ");
        section.push_str(method);
    }
    if let Some(decision_id) = relationship.explanation().decision_id() {
        section.push_str(" decision=");
        section.push_str(decision_id);
    }
    if let Some(caused_by_node_id) = relationship.explanation().caused_by_node_id() {
        section.push_str(" caused_by=");
        section.push_str(caused_by_node_id);
    }
    if let Some(sequence) = relationship.explanation().sequence() {
        section.push_str(" step=");
        section.push_str(&sequence.to_string());
    }

    section
}

pub(crate) fn render_detail(
    detail: &BundleNodeDetail,
    detail_by_node_id: &BTreeMap<&str, &BundleNodeDetail>,
) -> String {
    let revision = detail_by_node_id
        .get(detail.node_id())
        .map(|value| value.revision())
        .unwrap_or(detail.revision());

    format!(
        "Detail {} [rev {}]: {}",
        detail.node_id(),
        revision,
        detail.detail()
    )
}
