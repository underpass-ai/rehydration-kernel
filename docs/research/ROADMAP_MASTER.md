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

> **Snapshot-5 analysis (2026-03-28)** confirmed: N+1 batch loading and per-role
> recomputation are resolved. Architecture and command runtime are **green**.
> Main remaining debt is documentary/contractual, not structural.
> New items added: P0 beta-status.md drift, P2 README claim, query field pruning decision.

### P0 — Documentation drift

Active docs still reference `ContextAdminService` and admin path, removed in PR #57.

- [x] `api/proto/README.md:10` — remove `ContextAdminService` from public surface list
- [x] `README.md:34` — remove `admin` from `projection -> query -> compatibility -> command -> admin` journey
- [x] `docs/migration/kernel-node-centric-integration-contract.md:30` — remove `ContextAdminService` from services list

### P0 — beta-status.md factual drift (from snapshot-5 analysis 2026-03-28)

`docs/beta-status.md` still references `persist_snapshot` as accepted-but-ignored in
`UpdateContext`, and lists it in the ignored-fields table. That field was **removed**
from `command.proto` — these are now factual errors.

- [x] Remove `persist_snapshot` from UpdateContext description in beta-status.md
- [x] Remove `persist_snapshot` row from the ignored-fields table in beta-status.md
- [x] Grep confirmed: remaining `persist_snapshot` refs are in RehydrateSession (correct — field exists in query.proto)
- [x] Bonus: updated Path to v1 table (OTLP mTLS → Done, Grafana anonymous → Done, Neo4j → Partial)

### P0/P1 — Query contract overexpression (done)

8 deprecated proto fields pruned from v1beta1 (Option A — no consumers exist).
Field numbers reserved in proto. `Phase` and `ReplayMode` enums removed (dead code).
`BundleRenderFormat` retained (used in `RenderedContext.format`).

- [x] `GetContext`: removed `phase`, `work_item_id`, `render_format`, `include_debug_sections`
- [x] `ValidateScope`: removed `role`, `phase`
- [x] `RehydrateSession`: removed `include_timeline`, `include_summaries`
- [x] Proto: `reserved` field numbers + names on all three messages
- [x] Rust: all transport, agentic reference, reference client, test fixtures cleaned
- [x] `Phase` enum removed from `common.proto` (dead after field pruning)
- [x] `ReplayMode` enum removed from `common.proto` (never referenced anywhere)
- [x] clippy --tests -D warnings: zero warnings

### P1 — Performance hotspots

- [x] Batch `NodeDetailReader`: `load_node_details_batch()` port method + Valkey MGET adapter. Replaces N+1 sequential reads with a single multi-key fetch.
- [x] Cache graph reads across roles in `RehydrateSession`: `load_bundles_for_roles()` loads graph + details once, builds per-role bundles from shared data.
- [x] Performance observability: `QueryTimingBreakdown` (stdlib-only, no infra deps in application layer) captures graph_load / detail_load / bundle_assembly durations. Proto `QueryTimingBreakdown` message on `RehydrateSessionResponse`. OTel histograms: `rehydration.session.{graph_load,detail_load,bundle_assembly}.duration`, `role_count`, `batch_size`. Paper metrics extended with `graph_load_ms`, `detail_load_ms`, `bundle_assembly_ms`, `detail_batch_size`.

### P2 — README benchmark claim (from snapshot-5 analysis 2026-03-28)

README describes the LLM E2E benchmark as "the primary validation of the kernel's value".
Given current benchmark methodology debt, this overclaims. Soften to reflect directional
evidence rather than definitive validation.

- [x] Soften testing.md claim to "primary empirical validation harness" with methodology debt link
- [x] Soften README.md claim (same language) — commit d62408d

### P2 — Planner enrichment

`mode_heuristic.rs` now uses token pressure + causal density (planner v2, commit 6d99eb4).

- [x] Causal density: `>= 0.5` keeps ReasonPreserving even under budget pressure
- [ ] Endpoint type (GetContext vs RehydrateSession may warrant different mode defaults)
- [ ] Focus/path presence (if focus node exists, ResumeFocused may be better even with budget room)
- [ ] Relation distribution (structural-heavy graphs may benefit from pruning even at generous budgets)
- [ ] **Truncation algorithm upgrade**: current greedy packing drops sections that don't fit.
  Evaluate: tiered budget allocation (L0 guaranteed, L1 prioritized, L2 sacrificed),
  knapsack optimization, and `rationale_summary` field on relationships for graceful
  degradation without LLM-in-the-loop. Sliding window + summary ruled out (requires
  LLM call inside kernel, breaks determinism, introduces fabrication risk).
- [ ] Benchmark with `BENCHMARK_TOKEN_BUDGET=512` to exercise planner under real pressure (4096/49=83 tok/node doesn't trigger the heuristic)

Planner v2 A/B results (36 evals, budget=4096, no pressure activated):
- micro: explanatory 5/6 reason vs structural 0/6 (gap holds)
- stress: explanatory 3/6 reason vs structural 0/6 (gap holds, degrades with chain depth)
- All modes: reason_preserving (budget too generous to trigger switch)

## Pending — Architecture (low priority, all test-only)

| Task | File | Lines | Action |
|------|------|-------|--------|
| Split transport tests | transport/tests.rs | 902 | Separate files by feature (tests only, no prod impact) |
| Extract RESP protocol | adapter-valkey/io.rs | 663 | Shared module for RESP encoding |
| Extract TLS config | transport/grpc_server.rs | 222 | Separate TLS module |

### P1 — async-nats version conflict in TLS test fixtures (done)

- [x] Bump `rehydration-tests-kernel` dev-dependency from async-nats 0.39 → 0.46
- [x] `cargo clippy --workspace --all-targets --all-features -D warnings` passes clean

### P2 — Migrate explanatory_data.rs to dataset generator

`rehydration-tests-shared/src/seed/explanatory_data.rs` (1268 lines, 0% unit coverage)
is the hand-written predecessor to `dataset_generator.rs`. Still used by 3 tests:

- `relationship_use_case_integration.rs`
- `tier_resolution_integration.rs`
- `use_case_harness.rs` (paper metrics)

Migrate these to parameterized seeds from the generator, then delete the file.
Excluded from SonarCloud coverage gate until migrated.

### P2 — SonarCloud quality gate (from session 2026-03-28)

Quality gate FAILED: reliability C (1 bug), new code coverage 72.4% (target 80%).
Contract job temporarily removed from CI (proto pruning baseline).

- [ ] Fix bug: `extract-e2e-report.py:772` — conditional returns same value
- [ ] Reduce cognitive complexity: `llm_judge_prompt_evaluation.rs` (206), `vllm_benchmark_integration.rs` (135), `dataset_generator.rs` (37)
- [ ] Re-enable contract job once breaking change window closes (restore from commit af4c6a6)

### Coverage gaps (accepted, documented)

| File | Unit test | IT coverage | Reason |
|------|-----------|-------------|--------|
| `observability/src/lib.rs` | 19% | Deployed server | Global subscriber, single init per process |
| `observability/src/metrics.rs` | 49% | Noop meter unit test | Export only with real OTLP collector |
| `adapter-nats/context_event_store.rs` | 0% (unit) | 3 container tests | I/O boundary, requires JetStream |

## Done — Documentation 2.0 (audit 2026-03-27)

- [x] Archive 8 legacy compatibility docs to `docs/archived/`
- [x] Move 13 research/paper/incident docs to `docs/research/`
- [x] Remove starship_cluster_journey binary, scripts, demo docs
- [x] Remove vestigial `admin_bind` from config, Dockerfile, Helm, all call sites
- [x] Create `docs/testing.md` — unified test guide (270 unit, 9 integration, 4 benchmark)
- [x] Create `docs/observability.md` — quality metrics, OTel, Loki, per-RPC metric matrix
- [x] Rewrite `docs/usage-guide.md` — correct AsyncAPI event format, Operations domain examples, all RPCs
- [x] Rewrite `docs/operations/container-image.md` — separate image ENV vs runtime ENV, document both binaries, remove Helm duplication
- [x] Update `docs/beta-status.md` — async contract, path to v1, quality metrics, proto field matrix
- [x] Update `docs/security-model.md` — OTLP plaintext gap, Grafana anonymous admin, Neo4j no client mTLS, idempotency clarification
- [x] Update `README.md` — semantic relationships, multi-resolution tiers, security table, mermaid diagrams
- [x] Rename `docs/README.md` → `docs/index.md`
- [x] Remove `config.adminBind` from Helm values.yaml
- [x] Document OTel Collector → Loki dependency in kubernetes-deploy.md

## Pending — Technical debt (from audit 2026-03-27)

### P1 — Event store atomic concurrency (CAS)

Both NATS and Valkey event stores use check-then-act for optimistic concurrency:
`current_revision()` → compare → `publish()`. This is **not atomic** — two
concurrent clients can both load revision N, both pass the check, and both
append as N+1. The second write overwrites the first silently.

Fix: use JetStream's `expected_last_subject_sequence` in publish options.
The server rejects if sequence doesn't match, making the operation CAS.

- [ ] Add `Publish::expected_last_subject_sequence()` to NATS append
- [ ] Return `PortError::Conflict` when JetStream rejects sequence mismatch
- [ ] Add equivalent CAS to Valkey store (WATCH/MULTI/EXEC or Lua script)
- [ ] Add concurrent append integration test that exercises the race

### P2 — Idempotency outcome reliability

Idempotency outcome publish was silently ignored (`let _ = ...`). If event
appends but outcome publish fails, retries were treated as new requests.

- [x] Log warning on idempotency outcome publish failure (tracing::warn with key + error)
- [ ] Consider retry with backoff for outcome publish
- [x] Document retry semantics for consumers (at-least-once with idempotency key) — usage-guide.md

### P2 — Restructure docs/research/ index

Current README.md is a flat list. Needs proper classification of what is
active vs completed vs historical, and honest status per document.

- [ ] Classify each doc: active, completed, historical
- [ ] Add status and last-verified date per entry
- [ ] Remove or archive docs that are no longer relevant

### P2 — Write missing ADRs (1-6)

Six architecture decisions were made in PRs but never recorded as ADRs.
See [`docs/adr/README.md`](../adr/README.md) for the list and source PRs.

- [ ] ADR-001: Command/query separation + AsyncAPI (PR #1)
- [ ] ADR-002: Node-centric projection model (PR #4-#6)
- [ ] ADR-003: Compatibility bridge and removal (PR #8-#12, #52)
- [ ] ADR-004: Full TLS/mTLS on all boundaries (PR #32-#36)
- [ ] ADR-005: v1alpha1 removal — clean v1beta1 cut (PR #52, #57)
- [ ] ADR-006: Multi-resolution tiers + RehydrationMode (PR #63, #64)

### P1 — Testkit raw dump must use domain VO

`render_raw_dump()` in `raw_dump.rs` duplicates the raw text logic from
`BundleQualityMetrics::compute()` in the domain layer. Both were manually
aligned (bugs 1-3), but they can drift silently. `compression_ratio`
depends on both producing identical output.

- [x] Convert `GeneratedSeed` → `RehydrationBundle` in testkit (`seed_to_bundle()` mapper)
- [x] Use `BundleQualityMetrics::compute(bundle, 0, estimator).raw_equivalent_tokens()` via `seed_raw_equivalent_tokens()`
- [x] Verify raw dump token counts match before/after (test: `quality_metrics_match_between_domain_and_old_raw_dump`)
- [x] Delete `render_raw_dump()` — replaced by `count_raw_tokens()` via domain VO
- [x] All callers migrated to `seed_to_bundle()` + `BundleQualityMetrics::compute()`

### P1 — Quality metrics in RehydrateSession

`RehydrateSession` returns raw `Vec<RehydrationBundle>` without `RenderedContext`.
Quality metrics (`compression_ratio`, `causal_density`, etc.) are only emitted
by `GetContext` and `GetContextPath`. This means multi-role session consumers
get zero observability on render quality.

Fix requires rendering per-role bundles at the session level — architecture change:

- [x] Add `RenderedContext` per role to `RehydrateSessionResult` — `rendered_contexts: Vec<RenderedContext>`
- [x] Call `BundleQualityMetrics::compute()` for each role bundle — via `render_graph_bundle()`
- [x] Emit quality metrics via observer with `rpc=RehydrateSession` — per-role in handler
- [x] Proto: add `rendered` field (6) to `GraphRoleBundle`
- [ ] Update beta-status.md when done (currently listed as limitation)

### P1 — OTel metric parity across RPCs

`GetContext` emits 8 inline metrics (rpc.duration, bundle.nodes, bundle.relationships,
rendered.tokens, truncation.total, mode.selected + timing). `GetContextPath` only
emits rpc.duration + quality + timing. `RehydrateSession` only emits rpc.duration + timing.

- [x] `GetContextPath`: add bundle.nodes, bundle.relationships, bundle.details, rendered.tokens, truncation.total, mode.selected
- [x] `RehydrateSession`: add bundle metrics per role — nodes, rels, details, tokens, truncation, mode, quality observer
- [x] Wire `rehydration.bundle.details` — now recorded in GetContext
- [ ] Wire `rehydration.projection.lag` — defined in KernelMetrics but never recorded by projection runtime

### P2 — Async quality observer fan-out

`CompositeQualityObserver` calls all adapters synchronously in the gRPC handler
hot path. If an adapter blocks (network I/O, slow Loki push), it adds latency
to every `GetContext` response.

- [ ] Evaluate `tokio::spawn` for observer calls (fire-and-forget)
- [ ] Or buffer in-memory and flush on a background interval
- [ ] Measure overhead: current sync fan-out vs async with tracing + OTel noop meter

## Pending — Security hardening (from audit 2026-03-27)

### P1 — OTLP mTLS

Kernel → OTel Collector is plaintext gRPC (`with_tonic()` without TLS config).
Production deployments must co-locate or network-isolate the collector.

- [x] Add TLS config to OTLP exporter in `init_otel_tracer()` and `init_otel_metrics()` via env vars
- [x] Helm values: `otelCollector.tls.enabled`, `otelCollector.tls.existingSecret` with cert/key/ca
- [x] OTel Collector receiver: mTLS with client_ca_file when tls.enabled
- [x] OTel Collector → Loki exporter: mTLS with cert+key+ca, HTTPS endpoint

### P1 — Grafana anonymous admin

Helm chart deploys Grafana with `GF_AUTH_ANONYMOUS_ENABLED=true` and
`GF_AUTH_ANONYMOUS_ORG_ROLE=Admin`. Acceptable for development, not for production.

- [x] Change Helm default to `GF_AUTH_ANONYMOUS_ENABLED=false`
- [x] Add `grafana.anonymousAccess` toggle in values.yaml (default: false)
- [ ] Document production Grafana configuration in operations guide

### P2 — Neo4j client mTLS

Neo4j supports mTLS since v5.19+ (client certificate as 2FA). The `neo4rs`
driver's `with_client_certificate()` can load client certs. Currently our
adapter only passes a CA path for server verification.

- [x] Add `neo4jTls.keys.cert` and `neo4jTls.keys.key` to Helm values
- [x] Parse `tls_cert_path` + `tls_key_path` in Neo4j endpoint (with pair validation)
- [x] Helm helper appends cert+key query params to graph URI
- [ ] Pass client cert+key to `ConfigBuilder` (neo4rs 0.8 only has `with_client_certificate` for CA; needs upgrade or manual rustls config)
- [ ] Test with mTLS-enabled Neo4j container

### P2 — Vestigial `admin_bind` config field

Removed in this session. Verify no downstream consumers depend on `REHYDRATION_ADMIN_BIND`.

- [x] Remove `admin_bind` from `AppConfig` struct and all call sites
- [x] Remove `REHYDRATION_ADMIN_BIND` from Dockerfile, Helm deployment, docs

## Pending — Testing

- [ ] End-to-end mTLS integration test: gRPC mutual TLS with container-backed Neo4j, Valkey, and NATS — all TLS-encrypted
- [ ] OTel collector integration test: container OTLP collector verifying trace and metric export
- [ ] OpenTelemetry instrumentation for vLLM server: E2E traces from kernel gRPC → render → vLLM inference → evaluation
- [ ] vLLM backpressure: rate limiting, queue depth monitoring, retry with backoff, circuit breaker for parallel/scale benchmarks
- [ ] LLM API backpressure: rate limiting and retry with exponential backoff for Anthropic/OpenAI APIs (429 handling, quota awareness, cost circuit breaker for full matrix runs)
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
- [x] Include resolved_mode in benchmark diagnostic output — persisted in result JSON

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

### Phase-aware rehydration (future — concept valid, implementation pending)

Agent lifecycle phases (Discovery, Planning, Design, Build, Validate, Release) should
influence what context gets rehydrated and how it is prioritized. The concept was part
of the original proto design (commit `11479f5`, 2026-03-07) but was never implemented
— the `Phase` enum carried through v1alpha1 → v1beta1 as a no-op and was pruned in
the contract cleanup (2026-03-28).

The idea remains sound: a Build-phase agent needs implementation detail and dependency
context; a Discovery-phase agent needs broader exploratory context with less depth.

- [ ] Define phase-specific salience profiles (which semantic classes, relation types, and detail levels each phase prioritizes)
- [ ] Add `phase` back to `GetContextRequest` when implementation is ready (new field number, not reusing reserved 3)
- [ ] Phase × RehydrationMode interaction: phase may influence the mode heuristic (e.g., Discovery → ReasonPreserving even under pressure)
- [ ] Phase-aware planner: feed phase signal into `mode_heuristic.rs` alongside token pressure, causal density, endpoint type

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
[`docs/incident-report-e2e-ground-truth-2026-03-26.md`](incidents/incident-report-e2e-ground-truth-2026-03-26.md).

- [x] `expected_failure_point`: leaf node (deepest chain) — rewards L1 tracing over L0 surface
- [x] `expected_restart_node`: causal predecessor from seed topology
- [x] `expected_reason`: concatenate all chain rationales
- [x] Re-run diagnostic — frontier models score >= small models (confirmed)
- [x] Strip markdown code fences before parsing LLM responses (strip_markdown_fences in llm_evaluator)
- [x] Strict/lenient judges converge (±1 on all metrics, validated 2026-03-26)
- [x] Re-run full 720-eval matrix with fixed ground truth (2026-03-26: 720 evals, explanatory 62% Task vs structural 18%)

### Benchmark methodology redesign (from external review 2026-03-27)

External technical review of the 2026-03-26 benchmark identified 5 methodological
weaknesses. See [`docs/benchmark-2026-03-26-technical-review.md`](incidents/benchmark-2026-03-26-technical-review.md).

**P0 — Structural contamination** (blocks paper claims):
- [x] Fix A: structural variants must have ZERO rationale in ALL branches (main, noise, competing). Change `dataset_generator.rs`: when `relation_mix == Structural`, noise/distractor branches must not emit `rationale`, `method`, or `decision_id`. Currently structural+competing jumps from reason=0.8% to reason=64.2% due to distractor rationale leaking.
- [x] Fix B: split `reason` metric into `reason_correct_main_path` and `reason_plausible_but_wrong`. Requires new judge verdict fields. The current binary makes it impossible to distinguish correct preservation from distractor leakage.

**P1 — Replication** (blocks statistical claims):
- [x] Fix C: add `seeds_per_cell` to evaluation-matrix.yaml (default: 3). Seed rotates node-kind order and varies rationale text. Capture loop generates N structurally different graphs per cell. Report includes within-condition variance table when seeds > 1. Validated: 18-eval run with 3 seeds, different chain kinds per seed visible in logs.

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
- [x] L0/L1/L2 tier token desglose (persist `tier_tokens` in result JSON) — tier_l0/l1/l2/total_tokens
- [x] Token efficiency vs raw document dump baseline
- [ ] Detail presence impact (run with/without node details)
- [ ] Truncation impact at multiple token budgets (512, 1024, 2048, 4096)
- [ ] Time-to-first-token from API response headers

**P1 — Domain data not captured in benchmark results** (from session 2026-03-28):

The kernel domain exposes these in the gRPC response but the tests don't persist them:

- [x] `resolved_mode` (RehydrationMode selected by planner) — in `rendered.resolved_mode`
- [x] Per-tier token counts (L0/L1/L2 + tier_total_tokens) — in `rendered.tiers[].token_count`
- [x] `QueryTimingBreakdown` (graph_load, detail_load, bundle_assembly, batch_size) — in `response.timing`
- [x] `llm_prompt_tokens` + `llm_completion_tokens` — from `LlmEvaluationResult`
- [ ] `served_at` timestamp — in `response.served_at`

**P1 — TruncationMetadata not in proto** (domain gap):

`TruncationMetadata` (budget_requested, budget_used, sections_kept/dropped) exists in the
application layer (`bundle_truncator.rs`) but is not mapped to the proto response. The test
cannot capture it until it's exposed. Blocks "truncation impact" metric above.

- [x] Add `TruncationMetadata` message to `common.proto`
- [x] Add `truncation` field (8) to `RenderedContext` proto message
- [x] Map application `TruncationMetadata` → proto in `rendered_mapping.rs`
- [ ] Capture truncation fields in benchmark results (available in proto, not yet in BenchmarkResult)

**P2 — AsyncAPI `context.bundle.generated` has no quality data**:

The only kernel async output carries zero observability: no quality metrics, timing,
mode, or truncation. Downstream NATS consumers get revision+hash only.

- [ ] Add quality metrics fields to `ContextBundleGeneratedData` schema
- [ ] Add `resolved_mode`, `rendered_tokens`, `compression_ratio` at minimum
- [ ] Implement the publish (currently contract-only, not emitted by runtime)

**P0 — BundleQualityMetrics bugs** (from audit 2026-03-27, blocks metric trust):
- [x] Fix raw dump parity: kernel `compute_quality_metrics()` missing `caused_by_node_id` field that testkit `raw_dump.rs` includes → `raw_equivalent_tokens` underestimated
- [x] Fix detail formatting: kernel uses `"Detail: {}.\n"` (newline), testkit uses `" Detail: {}."` (inline) → token count mismatch
- [x] Fix semantic class format: both now use `.as_str()` ("causal") — testkit changed from `{:?}` ("Causal")
- [x] Add unit tests for all 5 quality metrics in `render_graph_bundle.rs` (8 tests: positivity, compression ratio, causal density, noise ratio, detail coverage, caused_by_node_id inclusion, all-causal=1.0, no-rels=0.0)
- [x] Handle `quality = None` in E2E test explicitly with `.ok_or()` instead of `unwrap_or_default()` zeros
- [x] Emit quality OTel metrics in `get_context_path()` (5 histograms). Note: `rehydrate_session()` returns raw bundles without `RenderedContext` — quality OTel requires architecture change to add rendering to session path.

### Judge prompt redesign (from incident 2026-03-26)

Opus 4.6 as judge rejects 100% of verdicts that Opus 4 accepted at 94%.
Root cause: prompt designed for flat bundles + Opus 4 calibration. See
[`docs/incident-report-benchmark-2026-03-26.md`](incidents/incident-report-benchmark-2026-03-26.md).

- [x] Judge prompt v2: causal-chain-aware `task_correct` (accept causal ancestors) — Rule 2 in llm_prompts.yaml
- [x] Judge prompt v2: paraphrase-gradient `reason_preserved` (context-derived vs generic) — Rule 4 sub-rules in llm_prompts.yaml
- [ ] Per-use-case judge prompt variants (failure diagnosis, handoff, constraint have different criteria)
- [x] Log raw inference + judge responses in evaluator (done — `results/*.json` per eval)
- [x] Store `llm_response` in paper metrics for post-hoc analysis (done — `agent_response` + `judge_raw`)
- [x] Judge calibration pre-check: `calibrate_judge()` sends known-good and known-bad synthetic responses before any eval. Runs before container boot — zero waste on miscalibrated judges.
- [x] Pin judge model version in evaluation-matrix.yaml (date-suffixed IDs: claude-opus-4-6-20250514, etc.)
- [ ] Re-run full benchmark matrix after methodology fixes

## Core Thesis (directional evidence, 2026-03-28)

Two dimensions:

1. **Accuracy**: explanatory context (causal chains + rationale metadata) enables
   LLMs to perform bounded graph tasks that structural-only context cannot support.

2. **Auditability**: the kernel makes LLM reasoning verifiable. `causal_density > 0`
   means rationale exists in the graph. Consumers can cross-reference the LLM's
   declared `reason_source` against the kernel's ground truth to detect fabrication
   deterministically — no judge needed.

Early signal (smoke test, 6 evals, Qwen3-8B, not yet statistically robust):
- Explanatory preserves rationale; structural does not
- Fabrication detection fires correctly on structural variants
- Qwen3-8B declares `confidence: high` on fabricated rationale — model honesty
  varies by size (under investigation in model matrix)

Full validation pending: 108-eval baseline + multi-model matrix.

## Pending — Research

### Level 1 — Submission-ready
- ~~Freeze artifact~~ (done)
- ~~Meso graph~~ (done for UC1)
- ~~detail_only baseline~~ (done)
- ~~retry_success_rate~~ (done)
- ~~Latency metrics in paper artifact~~ (done)
- [x] Expand meso to UC2-UC4

### Level 2 — Strong paper
- [x] Token efficiency baseline: BundleQualityMetrics in kernel render pipeline — raw_equivalent_tokens, compression_ratio, causal_density, noise_ratio, detail_coverage. Proto + OTel + E2E report. Kernel-native, not benchmark-computed.
- [ ] vLLM reasoning model: Qwen3-8B with `--reasoning-parser=qwen3` + `enable_thinking: true` per request. vLLM separates thinking into `reasoning_content` field — no stripping needed. `LLM_ENABLE_THINKING=true` env var activates per-request. 4x token budget for thinking overhead. Infrastructure ready, pending A/B comparison.
- [ ] Closed-loop recovery with corrected outcome
- ~~Three graph scales: micro, meso, stress~~ (done: dataset generator)
- [x] Noise controls: CompetingCausal mode — distractors with causal semantic classes and plausible rationale. Explanatory 100% unaffected, structural drops to 28%
- ~~Two domains minimum~~ (done: Operations + SoftwareDebugging)
- [ ] Pull and event-driven evaluation with same metrics
- [ ] External baseline families
- ~~vLLM in the loop tests~~ (done: 18-config benchmark with LLM-as-judge)
- ~~Dataset Generator~~ (done: micro/meso/stress × 2 domains)

### Level 2 — Small model capability matrix

The kernel's thesis is that **small models + structured rehydrated context** can perform
bounded graph tasks that normally require frontier models. The benchmark should prove:

1. Small model + kernel (explanatory) ≈ frontier model + raw context
2. Small model + kernel (explanatory) >> small model + kernel (structural)
3. The gap closes as kernel context quality improves (planner, tiers, noise pruning)

**Model matrix for vLLM (local GPU inference, zero API cost):**

Infrastructure: 4x RTX 3090 (96GB VRAM). vLLM tensor parallelism.

Small (1 GPU, fp16):
- [ ] Qwen3-1.7B — stress test: minimum viable model size
- [ ] Phi-4-mini (3.8B) — Microsoft, different architecture family
- [ ] Qwen3-4B — small but capable
- [ ] Gemma-3-4B-it — Google, different training data
- [ ] DeepSeek-R1-Distill-Qwen-7B — reasoning/CoT, tests if thinking + kernel compounds
- [x] Qwen3-8B — current baseline

Medium (1-2 GPUs, fp16):
- [ ] Qwen3-14B — sweet spot: single GPU, much more capable than 8B
- [ ] Mistral-Small-3.2-24B-Instruct — strong instruction follower
- [ ] Gemma-3-27B-it — largest single-architecture that fits in 2 GPUs

Large (2-4 GPUs, fp16):
- [ ] Qwen3-32B — 2 GPUs, tests if more params overcome structural-only context
- [ ] DeepSeek-R1-Distill-Qwen-32B — reasoning + 32B, strongest local reasoning model
- [ ] Llama-3.3-70B-Instruct — 4 GPUs fp16, frontier-class local model

MoE (fits easily despite large total params):
- [ ] Qwen3-30B-A3B — 30B total but only 3B active, very fast inference
- [ ] Qwen3-235B-A22B — 235B total, 22B active, fits in 4x24GB with 4-bit quant

Each model runs the same 108-eval config (~15 min per model, zero API cost).
The full matrix (15 models × 108 evals = 1620 evals) takes ~4 hours.

**Key questions:**
1. At what model size does the kernel stop compensating for model weakness?
2. Does reasoning (CoT) compound with structured context or is it redundant?
3. Do MoE models benefit differently from dense models at similar active param counts?
4. Is there a model where structural-only context matches explanatory? (kernel ceiling)

### P0 — Anti-fabrication: explicit "not available" + source declaration

The inference prompt forces the model to fill `reason` even when no rationale metadata
exists in the context. This causes fabrication. Two changes required before the next
exhaustive run:

**A — Explicit NOT_AVAILABLE path** (inference prompt change):
- [x] Add NOT_AVAILABLE instruction to inference prompt
- [x] Judge Rule 4 already handles empty ground truth correctly (no change needed)

**B — Source declaration + confidence** (response schema change):
- [x] Add `reason_source` field to inference prompt JSON schema
- [x] Add `confidence` field to inference prompt JSON schema
- [x] Update `LlmEvaluationResult`: `llm_reason_source`, `llm_confidence`, `llm_reason_fabricated`
- [x] Update `BenchmarkResult` in both tests
- [x] Fabrication detection in evaluator (deterministic, not judge): `reason_source == graph_metadata AND ground_truth.reason.is_none()`
- [x] `extract_source_fields()` parses reason_source/confidence from inference response JSON

**Validation (2026-03-28):** smoke test confirmed fabrication detection works.
Qwen3-8B claims `graph_metadata` + `confidence: high` on ALL variants including
structural (causal_density=0.0). The model ignores NOT_AVAILABLE instructions
entirely. `llm_reason_fabricated=true` fires correctly on both structural variants.

**Finding (Qwen3-8B):** small models are not honest about their reasoning source —
they declare high confidence in fabricated rationale.

**Finding (Qwen3-14B reasoning, 2026-03-29):** larger models with thinking ARE honest.
Qwen3-14B declares `not_available` + `low` confidence on structural variants
(zero fabrication). But overall accuracy drops (1/6 Task vs 3/6 for 8B) — the
thinking makes the model more cautious, possibly over-cautious.

Key questions remaining:
- Does prompt tuning recover the accuracy loss from thinking?
- Is the honesty/accuracy tradeoff consistent across model sizes?
- At what size does honesty emerge without thinking?

### Level 1 — Fabricated vs preserved rationale (research gap from 2026-03-28)

In the baseline run, structural variants show `restart_explained: true` even though
`reason_correct_main_path: false`. The LLM fabricates plausible-sounding justifications
("the incident root is the starting point of the causal chain") that are **not grounded
in graph metadata** — they are inference, not preservation.

The judge currently distinguishes these via two independent fields:
- `restart_explained`: did the model give ANY causal justification? (yes/no)
- `reason_correct_main_path`: does it match the actual rationale from the graph?

But this gap deserves deeper investigation:

- [ ] **Fabrication rate by mix**: how often does the model invent rationale when none exists (structural) vs cite real rationale (explanatory)? Measure `restart_explained=true AND reason_correct=false` as a proxy for fabrication.
- [ ] **Fabrication quality**: are fabricated rationales internally consistent? Do they use structural facts from the graph, or are they entirely hallucinated? Manual annotation on a sample.
- [ ] **Fabrication vs model size**: do larger models fabricate MORE convincingly (harder to detect) or LESS (they recognize absence of evidence)? This has safety implications.
- [ ] **Judge sensitivity to fabrication**: can the judge reliably distinguish "correct rationale cited from context" vs "plausible rationale invented by the model"? Test with adversarial fabrications.
- [ ] **New metric: `reason_fabricated`**: true when the model provides rationale that sounds plausible but has no grounding in the graph metadata. Requires cross-referencing the LLM response against the actual relationship rationale fields in the seed.

This is a critical finding: the kernel doesn't just improve accuracy — it provides
**verifiable grounding**. Without explanatory metadata, there is no way to distinguish
preserved knowledge from fabricated reasoning. The kernel makes rationale auditable.

**Infrastructure:** all models run on vLLM via `llm.underpassai.com`. Add model entries
to `evaluation-matrix.yaml` — the YAML-driven config makes this trivial.

### Level 3 — SOTA push
- [ ] Public benchmark
- [ ] External system comparisons (GraphRAG, plain RAG)
- [ ] Human/expert evaluation
- [ ] Multi-agent handoff benchmarks
