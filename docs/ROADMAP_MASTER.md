# Master Roadmap

Single source of truth for kernel maturity, technical debt, and next steps.

## Completed

### P0 — Stop the bleed (PR #53)
- [x] NOT_FOUND instead of placeholder when root node missing
- [x] Remove tautological scope validation from GetContext
- [x] Require snapshot_ttl when persist_snapshot=true
- [x] beta-status.md documenting RPC maturity

### P1 — Retrieval, rendering, observability (PR #53)
- [x] TokenEstimator trait in domain (ports pattern)
- [x] Salience-based packing (root > focus > explanatory relations > neighbors > details)
- [x] TruncationMetadata in RenderedContext
- [x] Structured logging with tracing + tracing-subscriber
- [x] All 11 gRPC handlers instrumented with tracing::instrument

### P2 — Command hardening (PR #54)
- [x] ContextEventStore port + ContextUpdatedEvent domain event
- [x] ValkeyContextEventStore adapter (optimistic concurrency + idempotency)
- [x] Real UpdateContext: revision check, idempotency dedup, content hash from payloads
- [x] PortError::Conflict → Status::aborted in gRPC
- [x] Remove rehydration-transport-http-admin crate
- [x] security-model.md (full TLS + mTLS, threat model)
- [x] InMemoryContextEventStore in testkit

### JetStream + tiktoken + OTel (PR #55)
- [x] NatsContextEventStore adapter (NATS JetStream, pure — no Valkey dependency)
- [x] Cl100kEstimator replacing CharDivFourEstimator (tiktoken-rs, cl100k_base BPE)
- [x] OpenTelemetry traces via OTLP (opentelemetry + tracing-opentelemetry)
- [x] THIRD_PARTY_NOTICES.md

### Event store wiring + metrics + tests + refactoring (PR #56)
- [x] Wire NatsContextEventStore in server composition root
- [x] Config switch: `REHYDRATION_EVENT_STORE_BACKEND=nats|valkey`
- [x] Integration test with NATS JetStream container (3 tests: append, conflict, idempotency)
- [x] OTel metrics: rpc_duration, bundle_nodes, bundle_relationships, rendered_tokens, truncation_total, projection_lag
- [x] Helm chart: `observability.logFormat`, `observability.otlpEndpoint`, `observability.serviceName`
- [x] Validated in cluster
- [x] Coverage tests: Cl100kEstimator, truncation metadata, revision conflict gRPC mapping
- [x] SonarCloud coverage exclusions for runtime init (covered by IT)
- [x] Refactor: split render_graph_bundle.rs (535 → 4 files)
- [x] Refactor: split testkit/lib.rs (623 → 3 files)

### Honesty pass (PR #57)
- [x] Fix BundleSection.token_count: cl100k_base everywhere (was split_whitespace in proto mapping)
- [x] Remove admin gRPC service entirely (admin.proto, 5 RPCs, all use cases, -1782 lines)
- [x] Validate expected_content_hash in UpdateContext (returns Conflict on mismatch)
- [x] Salience by semantic_class: causal > motivational > evidential > constraint > procedural > structural
- [x] Persist full ContextUpdatedEvent as JSON (was revision+hash only)
- [x] Delete stale BUG_DEPTH_TRAVERSAL.md
- [x] Update beta-status.md: document ignored proto fields, remove admin section
- [x] 3 evidence tests: content_hash validation, causal-before-structural, cl100k_base per-section

## Pending — Hardening (from external review 2026-03-25)

### P0 — Documentation drift

Active docs still reference `ContextAdminService` and admin path, removed in PR #57.

- [x] `api/proto/README.md:10` — remove `ContextAdminService` from public surface list
- [x] `README.md:34` — remove `admin` from `projection -> query -> compatibility -> command -> admin` journey
- [x] `docs/migration/kernel-node-centric-integration-contract.md:30` — remove `ContextAdminService` from services list

### P0/P1 — Query contract overexpression

Proto fields accepted but ignored in runtime. Marked `[deprecated = true]` in proto.

- [x] `GetContext`: `phase`, `work_item_id`, `render_format`, `include_debug_sections` — deprecated in proto
- [x] `ValidateScope`: `role`, `phase` — deprecated in proto
- [x] `RehydrateSession`: `include_timeline`, `include_summaries` — deprecated in proto

Documented in `beta-status.md`. Fields remain in the wire format for backward compatibility but are explicitly deprecated.

### P1 — Performance hotspots

- [x] Batch `NodeDetailReader`: `load_node_details_batch()` port method + Valkey MGET adapter. Replaces N+1 sequential reads with a single multi-key fetch.
- [x] Cache graph reads across roles in `RehydrateSession`: `load_bundles_for_roles()` loads graph + details once, builds per-role bundles from shared data.
- [x] Performance observability: `QueryTimingBreakdown` (stdlib-only, no infra deps in application layer) captures graph_load / detail_load / bundle_assembly durations. Proto `QueryTimingBreakdown` message on `RehydrateSessionResponse`. OTel histograms: `rehydration.session.{graph_load,detail_load,bundle_assembly}.duration`, `role_count`, `batch_size`. Paper metrics extended with `graph_load_ms`, `detail_load_ms`, `bundle_assembly_ms`, `detail_batch_size`.

### P2 — Planner enrichment

`mode_heuristic.rs` uses a single signal (`tokens_per_node < 30`). Incorporate:

- [ ] Endpoint type (GetContext vs RehydrateSession may warrant different mode defaults)
- [ ] Focus/path presence (if focus node exists, ResumeFocused may be better even with budget room)
- [ ] Causal density (high explanatory relation ratio → prefer ReasonPreserving)
- [ ] Relation distribution (structural-heavy graphs may benefit from pruning even at generous budgets)

## Pending — Architecture (low priority, all test-only)

| Task | File | Lines | Action |
|------|------|-------|--------|
| Split transport tests | transport/tests.rs | 902 | Separate files by feature (tests only, no prod impact) |
| Extract RESP protocol | adapter-valkey/io.rs | 663 | Shared module for RESP encoding |
| Extract TLS config | transport/grpc_server.rs | 222 | Separate TLS module |

### Coverage gaps (accepted, documented)

| File | Unit test | IT coverage | Reason |
|------|-----------|-------------|--------|
| `observability/src/lib.rs` | 19% | Deployed server | Global subscriber, single init per process |
| `observability/src/metrics.rs` | 49% | Noop meter unit test | Export only with real OTLP collector |
| `adapter-nats/context_event_store.rs` | 0% (unit) | 3 container tests | I/O boundary, requires JetStream |

## Pending — Testing

- [ ] End-to-end mTLS integration test: gRPC mutual TLS with container-backed Neo4j, Valkey, and NATS — all TLS-encrypted
- [ ] OTel collector integration test: container OTLP collector verifying trace and metric export
- [ ] OpenTelemetry instrumentation for vLLM server: E2E traces from kernel gRPC → render → vLLM inference → evaluation
- [ ] vLLM backpressure: rate limiting, queue depth monitoring, retry with backoff, circuit breaker for parallel/scale benchmarks
- [x] Refine LLM-as-judge prompt: domain-aware ground truth, strict rationale preservation vs inference, lenient on IDs
- [x] Benchmark with frontier models for README: GPT-5.4 (OpenAI) inference + Claude Opus 4 (Anthropic) judge — 18 configs, explanatory 94% vs structural 61%
- [x] Externalize evaluation prompts to YAML (`resources/llm_prompts.yaml`) — overridable via `LLM_PROMPTS_PATH`
- [x] Multi-provider LLM support: OpenAI, OpenAI-new (GPT-5.x/o3/o4), Anthropic Claude
- [x] Cross-validation: Claude Opus 4 as inference + GPT-5.4 as judge — explanatory 83% vs structural 56%, gap consistent
- [x] Local model benchmark: vLLM Qwen3-8B inference + Claude Opus 4 judge — explanatory 100% vs structural 50%, gap 50pp (widest)

## Pending — Product evolution (from OSS improvement planning)

### Bundle multi-resolution (done — PR #63)
Tiered rendering alongside flat sections:
- **L0 Summary**: objective, status, blocker, next action (~100 tokens)
- **L1 Causal spine**: root, focus, causal/motivational/evidential relations (~500 tokens)
- **L2 Evidence pack**: structural relations, neighbors, details (remaining budget)
- Proto: `ResolutionTier` enum, `RenderedTier` message, `max_tier` on `GetContextRequest`
- Backward compatible: flat `content`/`sections` unchanged, `tiers` is additive
- 25 new tests (7 domain + 7 classifier + 5 render + 1 unit e2e + 5 container e2e)
- **Benchmark note**: tiers do not improve flat content scores — they are an output format
  for granular consumption (L0-only status checks, L1-only diagnosis), not a content
  selection improvement. The flat salience ordering already prioritizes the same content.

### RehydrationMode heuristic (done — PR #64)
Auto-detection resolves mode based on token pressure (tokens-per-node < 30 → ResumeFocused):
- **`resume_focused`**: prune distractors, causal spine only in L1, L2 dropped — fixes stress@512 from 0/3 to 3/3
- **`reason_preserving`**: default, all tiers populated (unchanged behavior)
- `temporal_delta`: placeholder, reserved
- `global_summary`: placeholder, reserved
- Benchmark evaluates tier L0+L1 content (hexagonal: consumer chooses interface)
- 13 new tests (4 domain + 5 heuristic + 3 classifier + 1 budget)

Observability (done):
- [x] Log resolved_mode in gRPC handler (tracing span field)
- [x] OTel metric: `rehydration.mode.selected` counter by mode
- [x] Proto: `RehydrationMode` enum, `rehydration_mode` on request, `resolved_mode` on response
- [ ] Include resolved_mode in benchmark diagnostic output (next iteration)

### Provenance and auditability (observability feature — done)
End-to-end: `source_kind`, `source_agent`, `observed_at` on nodes.
- [x] Domain: SourceKind enum + Provenance value object + BundleNode field
- [x] Application: event ingestion maps provenance, renderer surfaces it
- [x] Proto: SourceKind enum + Provenance message + GraphNode.provenance field
- [x] Transport: bidirectional mapping domain ↔ proto
- [x] Neo4j persistence: persist/read provenance fields on ProjectionNode
- [x] Differentiated provenance: human/agent/derived/projection by node role
- [x] Benchmarked: does not improve LLM accuracy (observability, not inference)
- [ ] Provenance on relationships (BundleRelationship + GraphRelationship)
- [ ] `derived_from`, `effective_at`, `staleness` (computed)
- [ ] `supports`, `contradicts` semantic classes

### Associative rehydration (low priority, high complexity)
Move beyond root + fixed depth:
- Anchor selection, subgraph scoring, guided expansion
- Heuristic PPR or salience-weighted traversal
- Soft pruning and diversity control

### Memory consolidation (future)
Evolve from rehydration to operational memory:
- Episodic / semantic / procedural layers
- Promotion of stable patterns, archival of old episodes
- Summary nodes, versioning, lineage

## Pending — Paper artifact

- ~~Recalculate paper metrics with cl100k_base tokenizer~~ (done)
- ~~Add latency capture to paper harness~~ (done)
- [x] Expand meso variants to UC2-UC4
- [ ] CI consistency check paper ↔ artifacts

### P0 — Ground truth construction is wrong (incident 2026-03-26)

Qwen3-8B scores 100% TaskOK while Opus 4.6 scores 58-97%. The ground truth
penalizes precise causal reasoning and rewards trivial root-matching. See
[`docs/incident-report-e2e-ground-truth-2026-03-26.md`](./incident-report-e2e-ground-truth-2026-03-26.md).

- [x] `expected_failure_point`: leaf node (deepest chain) — rewards L1 tracing over L0 surface
- [x] `expected_restart_node`: causal predecessor from seed topology
- [x] `expected_reason`: concatenate all chain rationales
- [x] Re-run diagnostic — frontier models score >= small models (confirmed)
- [x] Strip markdown code fences before parsing LLM responses (strip_markdown_fences in llm_evaluator)
- [x] Strict/lenient judges converge (±1 on all metrics, validated 2026-03-26)
- [x] Re-run full 720-eval matrix with fixed ground truth (2026-03-26: 720 evals, explanatory 62% Task vs structural 18%)

### Benchmark methodology redesign (from external review 2026-03-27)

External technical review of the 2026-03-26 benchmark identified 5 methodological
weaknesses. See [`docs/benchmark-2026-03-26-technical-review.md`](./benchmark-2026-03-26-technical-review.md).

**P0 — Structural contamination** (blocks paper claims):
- [x] Fix A: structural variants must have ZERO rationale in ALL branches (main, noise, competing). Change `dataset_generator.rs`: when `relation_mix == Structural`, noise/distractor branches must not emit `rationale`, `method`, or `decision_id`. Currently structural+competing jumps from reason=0.8% to reason=64.2% due to distractor rationale leaking.
- [x] Fix B: split `reason` metric into `reason_correct_main_path` and `reason_plausible_but_wrong`. Requires new judge verdict fields. The current binary makes it impossible to distinguish correct preservation from distractor leakage.

**P1 — Replication** (blocks statistical claims):
- [ ] Fix C: add `seeds_per_cell` to evaluation-matrix.yaml (default: 3). Loop the capture phase with different seeds per variant. 720 x 3 = 2160 evals per run. Enables within-condition variance estimation and robust confidence intervals.

**P1 — Model matrix balance** (blocks model comparison claims):
- [x] Fix D: equilibrate the model matrix. Added sonnet-4.6 as third judge (3 agents × 3 judges - 3 self = 6 combos). Report now includes controlled comparison tables: "Agent comparison (fixed judge)" and "Judge comparison (fixed agent)" for balanced claims.

**P2 — Noise taxonomy** (improves diagnostic power):
- [x] Fix E: replace binary `clean`/`competing` with granular noise taxonomy: `Structural` (baseline), `CompetingCausal` (plausible alt rationale), `ConflictingMainPath` (contradicts chain), `CompetingRestartPoint` (alt recovery node). Distributed across mixes to keep eval count flat (36 variants). Validated: restart noise drops restart accuracy to 0% while preserving reason_correct — targeted diagnostic power.

**P2 — Restart sub-metrics** (restart is the weakest metric at 34%):
- [x] Fix F: replace binary `restart_correct` with sub-metrics: `restart_exact`, `restart_off_by_one`, `restart_on_competing_branch`, `restart_explained`. Propagated through JudgeVerdict, LlmEvaluationResult, all 5 judge prompts, PaperUseCaseMetric, EvalResult, BenchmarkResult. Validated: qwen3-8b gets 0% exact but 67% off-by-one — restart failures are ±1 hop, not confusion.

**P1 — Scientific methodology** (blocks paper submission):
- [ ] Restructure report to follow A-J template from [`anexo-procedimiento-cientifico-analisis-datos.md`](./anexo-procedimiento-cientifico-analisis-datos.md): research question, hypothesis + H0, variables, experimental design, metrics, data quality, descriptive, uncertainty, threats to validity, interpretation, conclusion with limitations.
- [ ] Document explicit H0 for each hypothesis (null: no difference between explanatory and structural)
- [ ] Add limitations section: single-run, synthetic graphs, judge sensitivity, structural contamination
- [ ] Separate observation / interpretation / conclusion in all report sections

**Pending metrics** (need test code changes):
- [ ] L0/L1/L2 tier token desglose (persist `tier_tokens` in result JSON)
- [ ] Token efficiency vs raw document dump baseline
- [ ] Detail presence impact (run with/without node details)
- [ ] Truncation impact at multiple token budgets (512, 1024, 2048, 4096)
- [ ] Time-to-first-token from API response headers

### Judge prompt redesign (from incident 2026-03-26)

Opus 4.6 as judge rejects 100% of verdicts that Opus 4 accepted at 94%.
Root cause: prompt designed for flat bundles + Opus 4 calibration. See
[`docs/incident-report-benchmark-2026-03-26.md`](./incident-report-benchmark-2026-03-26.md).

- [ ] Judge prompt v2: causal-chain-aware `task_correct` (accept causal ancestors)
- [ ] Judge prompt v2: paraphrase-gradient `reason_preserved` (context-derived vs generic)
- [ ] Per-use-case judge prompt variants (failure diagnosis, handoff, constraint have different criteria)
- [x] Log raw inference + judge responses in evaluator (done — `results/*.json` per eval)
- [x] Store `llm_response` in paper metrics for post-hoc analysis (done — `agent_response` + `judge_raw`)
- [ ] Judge calibration pre-check: validate known-good/known-bad before full benchmark
- [ ] Pin judge model version in paper methodology section (sensitivity finding)
- [ ] Re-run full benchmark matrix after methodology fixes

## Pending — Research

### Level 1 — Submission-ready
- ~~Freeze artifact~~ (done)
- ~~Meso graph~~ (done for UC1)
- ~~detail_only baseline~~ (done)
- ~~retry_success_rate~~ (done)
- ~~Latency metrics in paper artifact~~ (done)
- [x] Expand meso to UC2-UC4

### Level 2 — Strong paper
- [ ] Token efficiency baseline: measure rehydrated graph context tokens vs raw document dump for the same information. Proves the graph compresses context while preserving causal signal.
- [ ] vLLM reasoning model: replace Qwen3-8B with a reasoning-capable model (e.g. Qwen3-8B with thinking enabled, or DeepSeek-R1-Distill) to evaluate whether chain-of-thought improves causal tracing over the rehydrated graph.
- [ ] Closed-loop recovery with corrected outcome
- ~~Three graph scales: micro, meso, stress~~ (done: dataset generator)
- [x] Noise controls: CompetingCausal mode — distractors with causal semantic classes and plausible rationale. Explanatory 100% unaffected, structural drops to 28%
- ~~Two domains minimum~~ (done: Operations + SoftwareDebugging)
- [ ] Pull and event-driven evaluation with same metrics
- [ ] External baseline families
- ~~vLLM in the loop tests~~ (done: 18-config benchmark with LLM-as-judge)
- ~~Dataset Generator~~ (done: micro/meso/stress × 2 domains)

### Level 3 — SOTA push
- [ ] Public benchmark
- [ ] External system comparisons (GraphRAG, plain RAG)
- [ ] Human/expert evaluation
- [ ] Multi-agent handoff benchmarks
