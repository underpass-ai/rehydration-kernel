use prost_types::Timestamp;
use rehydration_application::{
    AskMemoryQuery, GetContextPathResult, GetContextResult, GetNodeDetailResult,
    InspectMemoryQuery, MemoryAnswerPolicy, MemoryCoordinateData, MemoryData, MemoryDimensionData,
    MemoryEntryData, MemoryEvidenceData, MemoryIngestCommand, MemoryIngestOutcome,
    MemoryProvenanceData, MemoryRelationData, TemporalIncludeOptions, TemporalMemoryQuery,
    TemporalMemoryResult, TraceMemoryQuery, WakeMemoryQuery,
};
use rehydration_domain::{
    DimensionScopeMode, DimensionSelection, DimensionSelectionMode, RehydrationBundle,
    ResolutionTier, TemporalCoordinate, TemporalCursor, TemporalDirection,
};
use rehydration_proto::v1beta1::{
    AcceptedCounts, AnswerPolicy as ProtoAnswerPolicy, AnswerReason, AskRequest, AskResponse,
    DimensionScopeMode as ProtoDimensionScopeMode, DimensionSelection as ProtoDimensionSelection,
    DimensionSelectionMode as ProtoDimensionSelectionMode, IngestRequest, IngestResponse,
    IngestedMemory, InspectInclude, InspectRequest, InspectResponse, InspectedLinks,
    InspectedObject, MemoryConfidence, MemoryDetailLevel, MemoryDimension, MemoryEvidence,
    MemoryProvenance, MemoryRelation, MemorySemanticClass, MemorySourceKind, Proof,
    TemporalCoordinate as ProtoTemporalCoordinate, TemporalDirection as ProtoTemporalDirection,
    TemporalEntry as ProtoTemporalEntry, TemporalInclude, TemporalLimit, TemporalMoveRequest,
    TemporalMoveResponse, TemporalNearRequest, TemporalState, TraceRequest, TraceResponse,
    WakeClaim, WakePacket, WakeRequest, WakeResponse,
};
use tonic::Status;

const UNIX_SORT_OFFSET: i64 = 100_000_000_000;

type ProtoMappingResult<T> = Result<T, Box<Status>>;

pub(crate) fn ingest_command_from_proto(
    request: IngestRequest,
) -> ProtoMappingResult<MemoryIngestCommand> {
    let memory = request
        .memory
        .ok_or_else(|| invalid_argument("memory is required"))?;

    Ok(MemoryIngestCommand {
        about: request.about,
        memory: MemoryData {
            dimensions: memory
                .dimensions
                .into_iter()
                .map(dimension_from_proto)
                .collect(),
            entries: memory.entries.into_iter().map(entry_from_proto).collect(),
            relations: memory
                .relations
                .into_iter()
                .map(relation_from_proto)
                .collect(),
            evidence: memory
                .evidence
                .into_iter()
                .map(evidence_from_proto)
                .collect(),
        },
        provenance: request.provenance.map(provenance_from_proto),
        idempotency_key: request.idempotency_key,
        dry_run: request.dry_run,
    })
}

pub(crate) fn ingest_response_from_outcome(outcome: MemoryIngestOutcome) -> IngestResponse {
    IngestResponse {
        summary: format!(
            "Ingested {} {}, {} {}, and {} {} for {}.",
            outcome.accepted.entries,
            plural(outcome.accepted.entries, "entry", "entries"),
            outcome.accepted.relations,
            plural(outcome.accepted.relations, "relation", "relations"),
            outcome.accepted.evidence,
            plural(outcome.accepted.evidence, "evidence item", "evidence items"),
            outcome.about
        ),
        memory: Some(IngestedMemory {
            about: outcome.about,
            memory_id: outcome.memory_id,
            accepted: Some(AcceptedCounts {
                entries: outcome.accepted.entries as u32,
                relations: outcome.accepted.relations as u32,
                evidence: outcome.accepted.evidence as u32,
            }),
            read_after_write_ready: outcome.read_after_write_ready,
        }),
        warnings: outcome.warnings,
    }
}

pub(crate) fn wake_query_from_proto(request: WakeRequest) -> ProtoMappingResult<WakeMemoryQuery> {
    let budget = request.budget.unwrap_or_default();
    Ok(WakeMemoryQuery {
        about: request.about.clone(),
        role: non_empty(request.role).unwrap_or_else(|| "agent".to_string()),
        intent: non_empty(request.intent)
            .unwrap_or_else(|| format!("continue from live kernel memory `{}`", request.about)),
        dimensions: domain_dimension_selection(request.dimensions)?,
        token_budget: if budget.tokens == 0 {
            1600
        } else {
            budget.tokens
        },
        depth: if budget.depth == 0 { 2 } else { budget.depth },
        max_tier: max_tier_from_detail(memory_detail_level(budget.detail)?),
    })
}

pub(crate) fn wake_response_from_result(intent: &str, result: GetContextResult) -> WakeResponse {
    let relationships = memory_relations_from_bundle(&result.bundle);
    let evidence = memory_evidence_from_bundle(&result.bundle);
    let current_state = rendered_current_state(&result.rendered);
    let summary = rendered_summary(&result.rendered);

    WakeResponse {
        summary,
        wake: Some(WakePacket {
            objective: intent.to_string(),
            current_state,
            causal_spine: relationships
                .iter()
                .take(8)
                .map(|relationship| WakeClaim {
                    claim: format!("{} -> {}", relationship.source_ref, relationship.target_ref),
                    because: if relationship.why.is_empty() {
                        "Kernel relationship path selected this edge.".to_string()
                    } else {
                        relationship.why.clone()
                    },
                    evidence_ref: relationship.evidence.clone(),
                })
                .collect(),
            open_loops: Vec::new(),
            next_actions: Vec::new(),
            guardrails: Vec::new(),
        }),
        proof: Some(proof(
            relationships,
            evidence,
            Vec::new(),
            MemoryConfidence::Medium,
        )),
        warnings: Vec::new(),
    }
}

pub(crate) fn ask_query_from_proto(request: AskRequest) -> ProtoMappingResult<AskMemoryQuery> {
    let budget = request.budget.unwrap_or_default();
    Ok(AskMemoryQuery {
        about: request.about,
        question: request.question,
        answer_policy: answer_policy_from_proto(request.answer_policy)?,
        dimensions: domain_dimension_selection(request.dimensions)?,
        token_budget: if budget.tokens == 0 {
            2400
        } else {
            budget.tokens
        },
        depth: if budget.depth == 0 { 2 } else { budget.depth },
        max_tier: max_tier_from_detail(memory_detail_level(budget.detail)?),
    })
}

pub(crate) fn ask_response_from_result(
    question: &str,
    policy: MemoryAnswerPolicy,
    result: GetContextResult,
) -> AskResponse {
    let evidence = memory_evidence_from_bundle(&result.bundle);
    let answer = rendered_summary(&result.rendered);
    let because = evidence
        .iter()
        .take(5)
        .map(|item| AnswerReason {
            claim: item.source.clone(),
            evidence: item.text.clone(),
            r#ref: item.id.clone(),
        })
        .collect::<Vec<_>>();
    let confidence = if because.is_empty() {
        MemoryConfidence::Unknown
    } else {
        MemoryConfidence::Medium
    };

    let answer = match policy {
        MemoryAnswerPolicy::EvidenceOrUnknown if because.is_empty() => "UNKNOWN".to_string(),
        MemoryAnswerPolicy::EvidenceOrUnknown
        | MemoryAnswerPolicy::ShowConflicts
        | MemoryAnswerPolicy::BestEffort => answer,
    };

    AskResponse {
        summary: if answer.trim().is_empty() || answer == "UNKNOWN" {
            format!("No deterministic memory answer found for: {question}")
        } else {
            answer.clone()
        },
        answer,
        because,
        proof: Some(proof(
            memory_relations_from_bundle(&result.bundle),
            evidence,
            Vec::new(),
            confidence,
        )),
        warnings: Vec::new(),
    }
}

pub(crate) fn temporal_query_from_move_proto(
    request: TemporalMoveRequest,
    direction: TemporalDirection,
) -> ProtoMappingResult<TemporalMemoryQuery> {
    temporal_query(TemporalQueryParts {
        about: request.about,
        cursor: request.cursor,
        dimensions: request.dimensions,
        window: request.window,
        limit: request.limit,
        include: request.include,
        budget: request.budget,
        direction,
    })
}

pub(crate) fn temporal_query_from_near_proto(
    request: TemporalNearRequest,
) -> ProtoMappingResult<TemporalMemoryQuery> {
    temporal_query(TemporalQueryParts {
        about: request.about,
        cursor: request.around,
        dimensions: request.dimensions,
        window: request.window,
        limit: request.limit,
        include: request.include,
        budget: request.budget,
        direction: TemporalDirection::Near,
    })
}

pub(crate) fn temporal_response_from_result(
    requested_cursor: rehydration_proto::v1beta1::TemporalCursor,
    direction: TemporalDirection,
    result: TemporalMemoryResult,
) -> TemporalMoveResponse {
    let traversal = result.traversal;
    let entries = traversal
        .entries()
        .iter()
        .map(|entry| ProtoTemporalEntry {
            r#ref: entry.ref_id().to_string(),
            kind: entry.kind().to_string(),
            text: entry.text().to_string(),
            coordinates: entry
                .coordinates()
                .iter()
                .map(proto_coordinate_from_domain)
                .collect(),
        })
        .collect::<Vec<_>>();
    let selected_refs = entries
        .iter()
        .map(|entry| entry.r#ref.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let relationships = if result.include.relations {
        temporal_relations_from_bundle(&result.source_bundle, &selected_refs)
    } else {
        Vec::new()
    };
    let evidence = if result.include.evidence {
        temporal_evidence_from_bundle(&result.source_bundle, &selected_refs)
    } else {
        Vec::new()
    };
    let count = entries.len();

    TemporalMoveResponse {
        summary: format!(
            "Returned {count} temporal {}.",
            if count == 1 { "entry" } else { "entries" }
        ),
        temporal: Some(TemporalState {
            direction: proto_direction(direction) as i32,
            requested: Some(requested_cursor),
            resolved: Some(proto_coordinate_from_domain(traversal.resolved_cursor())),
        }),
        coverage: Some(rehydration_proto::v1beta1::TemporalCoverage {
            requested: Some(proto_dimension_selection_from_domain(
                traversal.requested_dimensions(),
            )),
            included: traversal.included_dimensions().to_vec(),
            missing: traversal.missing_dimensions().to_vec(),
        }),
        entries,
        proof: Some(proof(
            relationships,
            evidence,
            traversal.missing().to_vec(),
            if count == 0 {
                MemoryConfidence::Unknown
            } else {
                MemoryConfidence::Medium
            },
        )),
        warnings: Vec::new(),
    }
}

pub(crate) fn trace_query_from_proto(request: TraceRequest) -> TraceMemoryQuery {
    let budget = request.budget.unwrap_or_default();
    TraceMemoryQuery {
        from: request.from,
        to: request.to,
        role: non_empty(request.goal).unwrap_or_else(|| "tracer".to_string()),
        token_budget: if budget.tokens == 0 {
            1600
        } else {
            budget.tokens
        },
    }
}

pub(crate) fn trace_response_from_result(result: GetContextPathResult) -> TraceResponse {
    TraceResponse {
        summary: rendered_summary(&result.rendered),
        trace: memory_relations_from_bundle(&result.path_bundle),
        warnings: Vec::new(),
    }
}

pub(crate) fn inspect_query_from_proto(
    request: InspectRequest,
) -> ProtoMappingResult<InspectMemoryQuery> {
    let include = request.include.unwrap_or(InspectInclude {
        incoming: false,
        outgoing: false,
        details: true,
        raw: false,
    });
    validate_inspect_include(&include)?;
    Ok(InspectMemoryQuery {
        ref_id: request.r#ref,
        include_details: include.details,
    })
}

pub(crate) fn inspect_response_from_result(
    result: GetNodeDetailResult,
    include_details: bool,
) -> InspectResponse {
    let text = if include_details {
        result
            .detail
            .as_ref()
            .map(|detail| detail.detail.clone())
            .filter(|detail| !detail.trim().is_empty())
            .unwrap_or_else(|| result.node.summary.clone())
    } else {
        result.node.summary.clone()
    };
    let evidence = if include_details {
        result.detail.as_ref().map_or_else(Vec::new, |detail| {
            vec![MemoryEvidence {
                id: format!("detail:{}", detail.node_id),
                supports: vec![detail.node_id.clone()],
                text: detail.detail.clone(),
                source: detail.node_id.clone(),
                time: None,
                metadata: Default::default(),
            }]
        })
    } else {
        Vec::new()
    };

    InspectResponse {
        summary: format!("Found live kernel node `{}`.", result.node.node_id),
        object: Some(InspectedObject {
            r#ref: result.node.node_id,
            kind: result.node.node_kind,
            text,
        }),
        links: Some(InspectedLinks {
            incoming: Vec::new(),
            outgoing: Vec::new(),
        }),
        evidence,
        warnings: Vec::new(),
    }
}

struct TemporalQueryParts {
    about: String,
    cursor: Option<rehydration_proto::v1beta1::TemporalCursor>,
    dimensions: Option<ProtoDimensionSelection>,
    window: Option<rehydration_proto::v1beta1::TemporalWindow>,
    limit: Option<TemporalLimit>,
    include: Option<TemporalInclude>,
    budget: Option<rehydration_proto::v1beta1::MemoryBudget>,
    direction: TemporalDirection,
}

fn temporal_query(parts: TemporalQueryParts) -> ProtoMappingResult<TemporalMemoryQuery> {
    let cursor = parts
        .cursor
        .ok_or_else(|| invalid_argument("temporal cursor is required"))?;
    let budget = parts.budget.unwrap_or_default();
    let limit_entries = parts
        .limit
        .as_ref()
        .and_then(|limit| (limit.entries != 0).then_some(limit.entries as usize));
    let limit_tokens = parts
        .limit
        .as_ref()
        .and_then(|limit| (limit.tokens != 0).then_some(limit.tokens));
    let detail = memory_detail_level(budget.detail)?;
    Ok(TemporalMemoryQuery {
        about: parts.about,
        direction: parts.direction,
        cursor: domain_cursor_from_proto(&cursor)?,
        dimensions: domain_dimension_selection(parts.dimensions)?,
        window: parts
            .window
            .map(|window| {
                rehydration_domain::TemporalWindow::new(
                    window.before_entries as usize,
                    window.after_entries as usize,
                )
            })
            .unwrap_or_default(),
        limit_entries,
        include: parts
            .include
            .map(temporal_include_from_proto)
            .unwrap_or_default(),
        token_budget: if let Some(tokens) = limit_tokens {
            tokens
        } else if budget.tokens == 0 {
            2400
        } else {
            budget.tokens
        },
        depth: if budget.depth == 0 { 3 } else { budget.depth },
        max_tier: max_tier_from_detail(detail),
    })
}

fn dimension_from_proto(value: MemoryDimension) -> MemoryDimensionData {
    MemoryDimensionData {
        id: value.id,
        kind: value.kind,
        title: non_empty(value.title),
        metadata: value.metadata.into_iter().collect(),
    }
}

fn entry_from_proto(value: rehydration_proto::v1beta1::MemoryEntry) -> MemoryEntryData {
    MemoryEntryData {
        id: value.id,
        kind: value.kind,
        text: value.text,
        coordinates: value
            .coordinates
            .into_iter()
            .map(coordinate_from_proto)
            .collect(),
        metadata: value.metadata.into_iter().collect(),
    }
}

fn coordinate_from_proto(value: ProtoTemporalCoordinate) -> MemoryCoordinateData {
    MemoryCoordinateData {
        dimension: value.dimension,
        scope_id: value.scope_id,
        occurred_at: proto_timestamp_to_sort_string(value.occurred_at),
        observed_at: proto_timestamp_to_sort_string(value.observed_at),
        ingested_at: proto_timestamp_to_sort_string(value.ingested_at),
        valid_from: proto_timestamp_to_sort_string(value.valid_from),
        valid_until: proto_timestamp_to_sort_string(value.valid_until),
        sequence: value.sequence,
        rank: value.rank,
        metadata: value.metadata.into_iter().collect(),
    }
}

fn relation_from_proto(value: MemoryRelation) -> MemoryRelationData {
    let semantic_class = semantic_class_name(value.semantic_class());
    let confidence = confidence_name(value.confidence());

    MemoryRelationData {
        source_ref: value.source_ref,
        target_ref: value.target_ref,
        rel: value.rel,
        semantic_class,
        why: non_empty(value.why),
        evidence: non_empty(value.evidence),
        confidence,
        sequence: value.sequence,
    }
}

fn evidence_from_proto(value: MemoryEvidence) -> MemoryEvidenceData {
    MemoryEvidenceData {
        id: value.id,
        supports: value.supports,
        text: value.text,
        source: non_empty(value.source),
        time: proto_timestamp_to_sort_string(value.time),
        metadata: value.metadata.into_iter().collect(),
    }
}

fn provenance_from_proto(value: MemoryProvenance) -> MemoryProvenanceData {
    MemoryProvenanceData {
        source_kind: source_kind_name(value.source_kind()),
        source_agent: value.source_agent,
        observed_at: proto_timestamp_to_sort_string(value.observed_at)
            .unwrap_or_else(|| "unix:100000000000:000000000".to_string()),
        correlation_id: non_empty(value.correlation_id),
        causation_id: non_empty(value.causation_id),
    }
}

fn memory_relations_from_bundle(bundle: &RehydrationBundle) -> Vec<MemoryRelation> {
    bundle
        .relationships()
        .iter()
        .map(|relationship| {
            let explanation = relationship.explanation();
            MemoryRelation {
                source_ref: relationship.source_node_id().to_string(),
                target_ref: relationship.target_node_id().to_string(),
                rel: relationship.relationship_type().to_string(),
                semantic_class: proto_semantic_class(explanation.semantic_class()) as i32,
                why: explanation.rationale().unwrap_or_default().to_string(),
                evidence: explanation.evidence().unwrap_or_default().to_string(),
                confidence: proto_confidence(explanation.confidence()) as i32,
                sequence: explanation.sequence(),
            }
        })
        .collect()
}

fn memory_evidence_from_bundle(bundle: &RehydrationBundle) -> Vec<MemoryEvidence> {
    bundle
        .node_details()
        .iter()
        .map(|detail| MemoryEvidence {
            id: format!("detail:{}", detail.node_id()),
            supports: vec![detail.node_id().to_string()],
            text: detail.detail().to_string(),
            source: detail.node_id().to_string(),
            time: None,
            metadata: Default::default(),
        })
        .collect()
}

fn temporal_relations_from_bundle(
    bundle: &RehydrationBundle,
    selected_refs: &std::collections::BTreeSet<String>,
) -> Vec<MemoryRelation> {
    memory_relations_from_bundle(bundle)
        .into_iter()
        .filter(|relationship| {
            selected_refs.contains(&relationship.source_ref)
                || selected_refs.contains(&relationship.target_ref)
        })
        .collect()
}

fn temporal_evidence_from_bundle(
    bundle: &RehydrationBundle,
    selected_refs: &std::collections::BTreeSet<String>,
) -> Vec<MemoryEvidence> {
    let mut evidence_refs = selected_refs.clone();
    for relationship in bundle.relationships().iter().filter(|relationship| {
        relationship.relationship_type() == "supports"
            && selected_refs.contains(relationship.target_node_id())
    }) {
        evidence_refs.insert(relationship.source_node_id().to_string());
    }

    bundle
        .node_details()
        .iter()
        .filter(|detail| evidence_refs.contains(detail.node_id()))
        .map(|detail| MemoryEvidence {
            id: format!("detail:{}", detail.node_id()),
            supports: vec![detail.node_id().to_string()],
            text: detail.detail().to_string(),
            source: detail.node_id().to_string(),
            time: None,
            metadata: Default::default(),
        })
        .collect()
}

fn proof(
    path: Vec<MemoryRelation>,
    evidence: Vec<MemoryEvidence>,
    missing: Vec<String>,
    confidence: MemoryConfidence,
) -> Proof {
    Proof {
        path,
        evidence,
        conflicts: Vec::new(),
        missing,
        confidence: confidence as i32,
    }
}

fn rendered_summary(rendered: &rehydration_application::RenderedContext) -> String {
    rendered
        .tiers
        .iter()
        .find(|tier| !tier.content.trim().is_empty())
        .map(|tier| tier.content.clone())
        .or_else(|| {
            rendered
                .sections
                .iter()
                .find(|section| !section.content.trim().is_empty())
                .map(|section| section.content.clone())
        })
        .unwrap_or_else(|| rendered.content.clone())
}

fn rendered_current_state(rendered: &rehydration_application::RenderedContext) -> Vec<String> {
    let sections = rendered
        .sections
        .iter()
        .take(5)
        .map(|section| section.content.clone())
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    if sections.is_empty() && !rendered.content.trim().is_empty() {
        vec![rendered.content.clone()]
    } else {
        sections
    }
}

fn proto_coordinate_from_domain(coordinate: &TemporalCoordinate) -> ProtoTemporalCoordinate {
    ProtoTemporalCoordinate {
        dimension: coordinate.dimension().to_string(),
        scope_id: coordinate.scope_id().to_string(),
        occurred_at: timestamp_from_sort_or_rfc3339(coordinate.occurred_at()),
        observed_at: timestamp_from_sort_or_rfc3339(coordinate.observed_at()),
        ingested_at: timestamp_from_sort_or_rfc3339(coordinate.ingested_at()),
        valid_from: timestamp_from_sort_or_rfc3339(coordinate.valid_from()),
        valid_until: timestamp_from_sort_or_rfc3339(coordinate.valid_until()),
        sequence: coordinate.sequence(),
        rank: coordinate.rank(),
        metadata: Default::default(),
    }
}

fn proto_dimension_selection_from_domain(
    selection: &DimensionSelection,
) -> ProtoDimensionSelection {
    let mode = match selection.mode() {
        DimensionSelectionMode::All => ProtoDimensionSelectionMode::All,
        DimensionSelectionMode::Only => ProtoDimensionSelectionMode::Only,
        DimensionSelectionMode::Except => ProtoDimensionSelectionMode::Except,
    };
    ProtoDimensionSelection {
        mode: mode as i32,
        include: if selection.mode() == DimensionSelectionMode::Only {
            selection.dimensions().iter().cloned().collect()
        } else {
            Vec::new()
        },
        exclude: if selection.mode() == DimensionSelectionMode::Except {
            selection.dimensions().iter().cloned().collect()
        } else {
            Vec::new()
        },
        scope: proto_dimension_scope_mode(selection.scope_mode()) as i32,
        abouts: selection.abouts().iter().cloned().collect(),
    }
}

fn domain_dimension_selection(
    value: Option<ProtoDimensionSelection>,
) -> ProtoMappingResult<DimensionSelection> {
    let Some(value) = value else {
        return Ok(DimensionSelection::all());
    };
    let scope = value.scope();
    let abouts = value.abouts.clone();
    let selection = match value.mode() {
        ProtoDimensionSelectionMode::Only => {
            if value.include.is_empty() {
                return Err(invalid_argument(
                    "dimension selection mode ONLY requires include values",
                ));
            }
            if !value.exclude.is_empty() {
                return Err(invalid_argument(
                    "dimension selection mode ONLY must not set exclude values",
                ));
            }
            DimensionSelection::only(value.include)
        }
        ProtoDimensionSelectionMode::Except => {
            if value.exclude.is_empty() {
                return Err(invalid_argument(
                    "dimension selection mode EXCEPT requires exclude values",
                ));
            }
            if !value.include.is_empty() {
                return Err(invalid_argument(
                    "dimension selection mode EXCEPT must not set include values",
                ));
            }
            DimensionSelection::except(value.exclude)
        }
        ProtoDimensionSelectionMode::All | ProtoDimensionSelectionMode::Unspecified => {
            if !value.include.is_empty() || !value.exclude.is_empty() {
                return Err(invalid_argument(
                    "dimension selection mode ALL must not set include or exclude values",
                ));
            }
            DimensionSelection::all()
        }
    };
    apply_dimension_scope(selection, scope, abouts)
}

fn temporal_include_from_proto(value: TemporalInclude) -> TemporalIncludeOptions {
    TemporalIncludeOptions {
        evidence: value.evidence,
        relations: value.relations,
        raw_refs: value.raw_refs,
    }
}

fn apply_dimension_scope(
    selection: DimensionSelection,
    scope: ProtoDimensionScopeMode,
    abouts: Vec<String>,
) -> ProtoMappingResult<DimensionSelection> {
    let abouts = abouts
        .into_iter()
        .map(|about| about.trim().to_string())
        .filter(|about| !about.is_empty())
        .collect::<Vec<_>>();
    match scope {
        ProtoDimensionScopeMode::Unspecified | ProtoDimensionScopeMode::CurrentAbout => {
            if !abouts.is_empty() {
                return Err(invalid_argument(
                    "dimension scope CURRENT_ABOUT must not set abouts",
                ));
            }
            Ok(selection.with_current_about_scope())
        }
        ProtoDimensionScopeMode::Abouts => {
            if abouts.is_empty() {
                return Err(invalid_argument(
                    "dimension scope ABOUTS requires at least one about",
                ));
            }
            Ok(selection.with_about_scope(abouts))
        }
        ProtoDimensionScopeMode::AllAbouts => {
            if !abouts.is_empty() {
                return Err(invalid_argument(
                    "dimension scope ALL_ABOUTS must not set abouts",
                ));
            }
            Err(invalid_argument(
                "dimension scope ALL_ABOUTS requires global about index support",
            ))
        }
    }
}

fn proto_dimension_scope_mode(value: DimensionScopeMode) -> ProtoDimensionScopeMode {
    match value {
        DimensionScopeMode::CurrentAbout => ProtoDimensionScopeMode::CurrentAbout,
        DimensionScopeMode::Abouts => ProtoDimensionScopeMode::Abouts,
        DimensionScopeMode::AllAbouts => ProtoDimensionScopeMode::AllAbouts,
    }
}

fn validate_inspect_include(value: &InspectInclude) -> ProtoMappingResult<()> {
    if value.incoming || value.outgoing {
        return Err(invalid_argument(
            "inspect incoming/outgoing link expansion requires link-capable reader support",
        ));
    }
    if value.raw {
        return Err(invalid_argument(
            "inspect raw expansion is not available on the current typed response shape",
        ));
    }
    Ok(())
}

fn memory_detail_level(value: i32) -> ProtoMappingResult<MemoryDetailLevel> {
    MemoryDetailLevel::try_from(value)
        .map_err(|_| invalid_argument("memory budget detail is invalid"))
}

fn max_tier_from_detail(value: MemoryDetailLevel) -> Option<ResolutionTier> {
    match value {
        MemoryDetailLevel::Compact => Some(ResolutionTier::L0Summary),
        MemoryDetailLevel::Balanced => Some(ResolutionTier::L1CausalSpine),
        MemoryDetailLevel::Full => Some(ResolutionTier::L2EvidencePack),
        MemoryDetailLevel::Unspecified => None,
    }
}

fn answer_policy_from_proto(value: i32) -> ProtoMappingResult<MemoryAnswerPolicy> {
    Ok(
        match ProtoAnswerPolicy::try_from(value)
            .map_err(|_| invalid_argument("answer policy is invalid"))?
        {
            ProtoAnswerPolicy::Unspecified | ProtoAnswerPolicy::EvidenceOrUnknown => {
                MemoryAnswerPolicy::EvidenceOrUnknown
            }
            ProtoAnswerPolicy::ShowConflicts => MemoryAnswerPolicy::ShowConflicts,
            ProtoAnswerPolicy::BestEffort => MemoryAnswerPolicy::BestEffort,
        },
    )
}

fn domain_cursor_from_proto(
    value: &rehydration_proto::v1beta1::TemporalCursor,
) -> ProtoMappingResult<TemporalCursor> {
    let has_ref = !value.r#ref.trim().is_empty();
    let has_time = value.time.is_some();
    let has_sequence = value.sequence.is_some();
    if [has_ref, has_time, has_sequence]
        .into_iter()
        .filter(|present| *present)
        .count()
        != 1
    {
        return Err(invalid_argument(
            "temporal cursor requires exactly one of ref, time, or sequence",
        ));
    }

    if has_ref {
        return TemporalCursor::ref_id(value.r#ref.clone())
            .map_err(|error| invalid_argument(error.to_string()));
    }
    if let Some(time) = value.time {
        return TemporalCursor::time(
            proto_timestamp_to_sort_string(Some(time)).unwrap_or_default(),
        )
        .map_err(|error| invalid_argument(error.to_string()));
    }
    TemporalCursor::sequence(value.sequence.unwrap_or_default())
        .map_err(|error| invalid_argument(error.to_string()))
}

fn proto_direction(value: TemporalDirection) -> ProtoTemporalDirection {
    match value {
        TemporalDirection::Goto => ProtoTemporalDirection::Goto,
        TemporalDirection::Near => ProtoTemporalDirection::Near,
        TemporalDirection::Rewind => ProtoTemporalDirection::Rewind,
        TemporalDirection::Forward => ProtoTemporalDirection::Forward,
    }
}

fn proto_semantic_class(value: &rehydration_domain::RelationSemanticClass) -> MemorySemanticClass {
    match value {
        rehydration_domain::RelationSemanticClass::Structural => MemorySemanticClass::Structural,
        rehydration_domain::RelationSemanticClass::Causal => MemorySemanticClass::Causal,
        rehydration_domain::RelationSemanticClass::Motivational => {
            MemorySemanticClass::Motivational
        }
        rehydration_domain::RelationSemanticClass::Procedural => MemorySemanticClass::Procedural,
        rehydration_domain::RelationSemanticClass::Evidential => MemorySemanticClass::Evidential,
        rehydration_domain::RelationSemanticClass::Constraint => MemorySemanticClass::Constraint,
    }
}

fn proto_confidence(value: Option<&str>) -> MemoryConfidence {
    match value.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "high" => MemoryConfidence::High,
        "medium" => MemoryConfidence::Medium,
        "low" => MemoryConfidence::Low,
        _ => MemoryConfidence::Unknown,
    }
}

fn semantic_class_name(value: MemorySemanticClass) -> String {
    match value {
        MemorySemanticClass::Structural => "structural",
        MemorySemanticClass::Causal => "causal",
        MemorySemanticClass::Motivational => "motivational",
        MemorySemanticClass::Procedural => "procedural",
        MemorySemanticClass::Evidential => "evidential",
        MemorySemanticClass::Constraint => "constraint",
        _ => "",
    }
    .to_string()
}

fn confidence_name(value: MemoryConfidence) -> Option<String> {
    let value = match value {
        MemoryConfidence::High => "high",
        MemoryConfidence::Medium => "medium",
        MemoryConfidence::Low => "low",
        MemoryConfidence::Unknown => "unknown",
        _ => "",
    };
    non_empty(value.to_string())
}

fn source_kind_name(value: MemorySourceKind) -> String {
    match value {
        MemorySourceKind::Human => "human",
        MemorySourceKind::Agent => "agent",
        MemorySourceKind::Projection => "projection",
        MemorySourceKind::Derived => "derived",
        _ => "",
    }
    .to_string()
}

fn proto_timestamp_to_sort_string(value: Option<Timestamp>) -> Option<String> {
    let value = value?;
    Some(format!(
        "unix:{:012}:{:09}",
        value.seconds + UNIX_SORT_OFFSET,
        value.nanos.max(0)
    ))
}

fn timestamp_from_sort_or_rfc3339(value: Option<&str>) -> Option<Timestamp> {
    let value = value?;
    parse_unix_sort_timestamp(value).or_else(|| parse_basic_rfc3339(value))
}

fn parse_unix_sort_timestamp(value: &str) -> Option<Timestamp> {
    let suffix = value.strip_prefix("unix:")?;
    let (seconds, nanos) = suffix.split_once(':')?;
    Some(Timestamp {
        seconds: seconds.parse::<i64>().ok()? - UNIX_SORT_OFFSET,
        nanos: nanos.parse::<i32>().ok()?,
    })
}

fn parse_basic_rfc3339(value: &str) -> Option<Timestamp> {
    if value.len() < 20 || !value.ends_with('Z') {
        return None;
    }
    let year = value[0..4].parse::<i64>().ok()?;
    let month = value[5..7].parse::<u8>().ok()?;
    let day = value[8..10].parse::<u8>().ok()?;
    let hour = value[11..13].parse::<u8>().ok()?;
    let minute = value[14..16].parse::<u8>().ok()?;
    let second = value[17..19].parse::<u8>().ok()?;
    Timestamp::date_time(year, month, day, hour, minute, second).ok()
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn invalid_argument(message: impl Into<String>) -> Box<Status> {
    Box::new(Status::invalid_argument(message.into()))
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
