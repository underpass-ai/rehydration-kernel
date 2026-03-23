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

### Event store wiring + observability metrics + tests (PR #56)
- [x] Wire NatsContextEventStore in server composition root
- [x] Add config switch for event store backend (`REHYDRATION_EVENT_STORE_BACKEND=nats|valkey`)
- [x] Integration test with NATS JetStream container (3 tests: append, conflict, idempotency)
- [x] OTel metrics: rpc_duration, bundle_nodes, bundle_relationships, rendered_tokens, truncation_total, projection_lag
- [x] Helm chart: `observability.logFormat`, `observability.otlpEndpoint`, `observability.serviceName`
- [x] Validated in cluster (revision deployed, pod running)
- [x] Coverage tests: Cl100kEstimator, truncation metadata, revision conflict gRPC mapping
- [x] SonarCloud coverage exclusions for runtime init (observability, NATS adapter — covered by IT)

## Pending — Architecture refactoring (from audit)

### Immediate (high impact)

| Task | File | Lines | Action |
|------|------|-------|--------|
| Split render_graph_bundle.rs | application/queries/ | 535 | Extract to bundle_renderer, bundle_truncator, bundle_prioritizer |
| Split testkit/lib.rs | testkit/ | 623 | Docker setup → module, in-memory stores → separate files |
| Extract UpdateContext validators | application/commands/ | 347 | IdempotencyChecker, RevisionValidator, ContentHashCalculator |

### Short-term (medium impact)

| Task | File | Lines | Action |
|------|------|-------|--------|
| Split transport tests | transport/tests.rs | 902 | Separate files by feature |
| Extract RESP protocol | adapter-valkey/io.rs | 663 | Shared module for RESP encoding |
| Extract TLS config | transport/grpc_server.rs | 222 | Separate TLS module |

### Coverage gaps (accepted, documented)

| File | Unit test coverage | IT coverage | Reason for exclusion |
|------|-------------------|-------------|---------------------|
| `observability/src/lib.rs` | 19% | Not automated (global subscriber) | Runtime init, validated by deployed server |
| `observability/src/metrics.rs` | 49% | Noop meter unit test | Export only with real OTLP collector |
| `adapter-nats/context_event_store.rs` | 0% (unit) | 3 container tests | I/O boundary, requires JetStream |

## Pending — Product features

### Paper artifact
- [ ] Recalculate paper metrics with cl100k_base tokenizer
- [ ] Add latency capture to paper harness
- [ ] Expand meso variants to UC2-UC4
- [ ] CI consistency check paper ↔ artifacts

## Pending — Research (from ROADMAP_SOTA_CONTEXT_REHYDRATION.md)

### Level 1 — Submission-ready
- ~~Freeze artifact~~ (done)
- ~~Meso graph~~ (done for UC1)
- ~~detail_only baseline~~ (done)
- ~~retry_success_rate~~ (done)
- [ ] Latency metrics in paper artifact
- [ ] Expand meso to UC2-UC4

### Level 2 — Strong paper
- [ ] Closed-loop recovery with corrected outcome
- [ ] Three graph scales: micro, meso, stress
- [ ] Noise controls (distractors, competing motivations)
- [ ] Two domains minimum
- [ ] Pull and event-driven evaluation with same metrics
- [ ] External baseline families
- [ ] vLLM in the loop tests
- [ ] Dataset Generator

### Level 3 — SOTA push
- [ ] Public benchmark
- [ ] External system comparisons (GraphRAG, plain RAG)
- [ ] Human/expert evaluation
- [ ] Multi-agent handoff benchmarks
