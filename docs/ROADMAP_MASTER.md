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
- [ ] Cross-validation: Claude Opus 4 as inference + GPT-5.4 as judge (next iteration)

## Pending — Product evolution (from OSS improvement planning)

### Bundle multi-resolution (high priority, quick win)
Tiered rendering to replace uniform flat sections:
- **L0 Summary**: objective, status, blocker, next action
- **L1 Causal spine**: root, focus, top causal/motivational/evidential relations, resume path
- **L2 Evidence pack**: supporting details, errors, constraints, relevant data

### RehydrationMode heuristic (medium priority)
Deterministic mode selection based on query shape:
- `resume_focused`: optimize for restart point identification
- `reason_preserving`: optimize for rationale and motivation preservation
- `temporal_delta`: optimize for what changed since last rehydration
- `global_summary`: optimize for broad context overview

### Provenance and auditability (medium priority)
Metadata on nodes/relationships for trust and conflict detection:
- `source_kind`, `source_agent`, `derived_from`
- `observed_at`, `effective_at`, `confidence`, `staleness`
- `supports`, `contradicts`

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
- [ ] Expand meso variants to UC2-UC4
- [ ] CI consistency check paper ↔ artifacts

## Pending — Research

### Level 1 — Submission-ready
- ~~Freeze artifact~~ (done)
- ~~Meso graph~~ (done for UC1)
- ~~detail_only baseline~~ (done)
- ~~retry_success_rate~~ (done)
- ~~Latency metrics in paper artifact~~ (done)
- [ ] Expand meso to UC2-UC4

### Level 2 — Strong paper
- [ ] Closed-loop recovery with corrected outcome
- ~~Three graph scales: micro, meso, stress~~ (done: dataset generator)
- [ ] Noise controls (distractors, competing motivations)
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
