use rehydration_domain::{
    DimensionSelection, ResolutionTier, TemporalCursor, TemporalDirection, TemporalWindow,
};

use crate::queries::{GetNodeDetailResult, GraphRelationshipView};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIngestCommand {
    pub about: String,
    pub memory: MemoryData,
    pub provenance: Option<MemoryProvenanceData>,
    pub idempotency_key: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryData {
    pub dimensions: Vec<MemoryDimensionData>,
    pub entries: Vec<MemoryEntryData>,
    pub relations: Vec<MemoryRelationData>,
    pub evidence: Vec<MemoryEvidenceData>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MemoryDimensionData {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MemoryEntryData {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub coordinates: Vec<MemoryCoordinateData>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MemoryCoordinateData {
    pub dimension: String,
    pub scope_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub occurred_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ingested_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MemoryRelationData {
    #[serde(rename = "from")]
    pub source_ref: String,
    #[serde(rename = "to")]
    pub target_ref: String,
    pub rel: String,
    #[serde(rename = "class")]
    pub semantic_class: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MemoryEvidenceData {
    pub id: String,
    pub supports: Vec<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryProvenanceData {
    pub source_kind: String,
    pub source_agent: String,
    pub observed_at: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryAcceptedCounts {
    pub entries: usize,
    pub relations: usize,
    pub evidence: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIngestOutcome {
    pub about: String,
    pub memory_id: String,
    pub accepted: MemoryAcceptedCounts,
    pub read_after_write_ready: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WakeMemoryQuery {
    pub about: String,
    pub role: String,
    pub intent: String,
    pub dimensions: DimensionSelection,
    pub token_budget: u32,
    pub depth: u32,
    pub max_tier: Option<ResolutionTier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AskMemoryQuery {
    pub about: String,
    pub question: String,
    pub answer_policy: MemoryAnswerPolicy,
    pub dimensions: DimensionSelection,
    pub token_budget: u32,
    pub depth: u32,
    pub max_tier: Option<ResolutionTier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemporalMemoryQuery {
    pub about: String,
    pub direction: TemporalDirection,
    pub cursor: TemporalCursor,
    pub dimensions: DimensionSelection,
    pub window: TemporalWindow,
    pub limit_entries: Option<usize>,
    pub include: TemporalIncludeOptions,
    pub token_budget: u32,
    pub depth: u32,
    pub max_tier: Option<ResolutionTier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceMemoryQuery {
    pub from: String,
    pub to: String,
    pub role: String,
    pub token_budget: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectMemoryQuery {
    pub ref_id: String,
    pub include_details: bool,
    pub include_incoming: bool,
    pub include_outgoing: bool,
    pub include_raw: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct InspectMemoryResult {
    pub detail: GetNodeDetailResult,
    pub incoming: Vec<GraphRelationshipView>,
    pub outgoing: Vec<GraphRelationshipView>,
    pub include_details: bool,
    pub include_raw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemoryAnswerPolicy {
    #[default]
    EvidenceOrUnknown,
    ShowConflicts,
    BestEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TemporalIncludeOptions {
    pub evidence: bool,
    pub relations: bool,
    pub raw_refs: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemporalMemoryResult {
    pub traversal: rehydration_domain::TemporalTraversalResult,
    pub source_bundle: rehydration_domain::RehydrationBundle,
    pub include: TemporalIncludeOptions,
}
