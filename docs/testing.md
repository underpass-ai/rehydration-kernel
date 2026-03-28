# Testing Guide

270 unit tests, 9 integration tests (testcontainers), 4 benchmark tests (LLM-as-judge).

## Quick Start

```bash
# Unit tests (no infrastructure needed)
cargo test --workspace

# Integration tests (requires container runtime)
bash scripts/ci/testcontainers-runtime.sh
cargo test -p rehydration-tests-kernel --features container-tests -- --nocapture --test-threads=1

# Quality gate (pre-merge: format + clippy + contract + tests)
bash scripts/ci/quality-gate.sh
```

## Unit Tests

270 tests across the workspace. No external infrastructure needed.

| Crate | Tests | What it validates |
|:------|------:|:------------------|
| rehydration-application | 79 | Rendering pipeline, quality metrics, tier classification, mode heuristic |
| rehydration-domain | 41 | Value objects, invariants, BundleQualityMetrics, aggregate validation |
| rehydration-adapter-valkey | 38 | RESP protocol, endpoint parsing, TLS config, detail/snapshot stores |
| rehydration-testkit | 28 | Dataset generator, raw dump baseline, seed consistency |
| rehydration-transport-grpc | 24 | gRPC roundtrip, TLS/mTLS handshake, proto mapping |
| rehydration-adapter-neo4j | 15 | Endpoint parsing, TLS CA config, projection store |
| rehydration-proto | 12 | Contract stability, fixture compliance, AsyncAPI |
| rehydration-config | 10 | AppConfig defaults, gRPC TLS modes, NATS TLS validation |
| rehydration-observability | 5 | OTel/Tracing/Composite quality observers |
| rehydration-ports | 5 | Port trait delegation through Arc |

Run a specific crate:
```bash
cargo test -p rehydration-domain
```

## Integration Tests

Require `--features container-tests`. Testcontainers spins up Neo4j, Valkey, NATS automatically.

### Prerequisites

```bash
# Initialize container runtime (docker or podman)
bash scripts/ci/testcontainers-runtime.sh
```

### Test Matrix

| Test | What it validates | Infra | Script |
|------|-------------------|-------|--------|
| `kernel_full_journey_integration` | Projection → query → command full cycle | Neo4j + Valkey + NATS + gRPC | `scripts/ci/integration-kernel-full-journey.sh` |
| `kernel_full_journey_tls_integration` | mTLS end-to-end with generated certs | Same + OpenSSL certs | `scripts/ci/integration-kernel-full-journey-tls.sh` |
| `kernel_golden_integration` | Golden contract tests (4 RPCs) | Neo4j + Valkey + NATS + gRPC | — |
| `agentic_integration` | Agent using kernel context | Neo4j + Valkey + NATS + gRPC | `scripts/ci/integration-agentic-context.sh` |
| `agentic_event_integration` | Event-driven recording runtime | Neo4j + Valkey + NATS + gRPC | `scripts/ci/integration-agentic-event-context.sh` |
| `tier_resolution_integration` | Multi-resolution tiers L0/L1/L2 | Neo4j + Valkey + NATS + gRPC | — |
| `kernel_p1_performance_measurement` | Performance regression (batch vs N+1) | Neo4j + Valkey + NATS + gRPC | — |
| `relationship_use_case_integration` | 4 paper use cases (failure, handoff, constraint, pressure) | Neo4j + Valkey + NATS + gRPC | `scripts/ci/integration-paper-use-cases.sh` |
| `relationship_use_case_ablation_integration` | Ablation: degradation without explanatory relations | Same | Same script |

### Running individual tests

```bash
cargo test -p rehydration-tests-kernel \
  --features container-tests \
  --test kernel_full_journey_integration \
  -- --nocapture --test-threads=1
```

## Benchmark Tests (LLM-as-Judge)

Require API keys and/or vLLM endpoint. These are expensive — each evaluation calls an LLM for inference and a second LLM for judging.

### Configuration

All benchmark configuration lives in `crates/rehydration-testkit/resources/evaluation-matrix.yaml`.

### Tests

| Test | What it validates | Extra requirements | Estimated cost |
|------|-------------------|--------------------|----------------|
| `llm_judge_prompt_evaluation` | Full matrix: agents x judges x scales x noise | `ANTHROPIC_KEY`, `OPENAI_KEY` | ~$15 full matrix, ~$0.10 diagnostic |
| `vllm_benchmark_integration` | vLLM inference quality under budget pressure | vLLM endpoint (e.g. `llm.underpassai.com`) | GPU time only |

### Environment variables

| Variable | Required by | Description |
|----------|-------------|-------------|
| `ANTHROPIC_KEY` | llm_judge | Anthropic API key |
| `OPENAI_KEY` | llm_judge | OpenAI API key |
| `LLM_ENDPOINT` | both | Inference endpoint URL |
| `LLM_MODEL` | both | Model name (e.g. `Qwen/Qwen3-8B`) |
| `LLM_PROVIDER` | both | `openai`, `openai-new`, `anthropic` |
| `LLM_JUDGE_ENDPOINT` | llm_judge | Judge endpoint URL |
| `LLM_JUDGE_MODEL` | llm_judge | Judge model name |
| `LLM_JUDGE_PROVIDER` | llm_judge | Judge provider |
| `FILTER_SCALES` | both | Subset: `micro`, `meso`, `stress` |
| `FILTER_NOISE` | both | Subset: `clean`, `competing`, `conflicting`, `restart` |

### Quick diagnostic (cheapest path)

```bash
export ANTHROPIC_KEY="$(cat /tmp/claude.txt)"
export OPENAI_KEY="$(cat /tmp/openai.txt)"
bash scripts/ci/e2e-ground-truth-diagnostic.sh
```

### Full matrix

```bash
bash scripts/ci/evaluate-prompt-variants.sh
```

## Adapter Integration Tests

Isolated tests for individual adapters:

```bash
cargo test -p rehydration-adapter-neo4j --features container-tests --test neo4j_integration
cargo test -p rehydration-adapter-valkey --features container-tests --test valkey_integration
```

## CI Scripts Reference

| Script | Purpose |
|--------|---------|
| `scripts/ci/quality-gate.sh` | Pre-merge: format + clippy + contract + tests |
| `scripts/ci/contract-gate.sh` | Protobuf lint + breaking change detection |
| `scripts/ci/helm-lint.sh` | Helm chart validation (8 scenarios) |
| `scripts/ci/rust-coverage.sh` | LLVM coverage report including container tests |
| `scripts/ci/kubernetes-transport-smoke.sh` | In-cluster TLS/mTLS smoke test (gRPC + NATS + Valkey + OTel + Loki) |
| `scripts/ci/container-image.sh` | Build OCI container image |
| `scripts/ci/deploy-kubernetes.sh` | Helm deploy wrapper |

## Adding a New Test

Integration tests use the `TestFixture` builder pattern:

```rust
let fixture = TestFixture::builder()
    .with_neo4j()
    .with_valkey()
    .with_nats()
    .with_projection_runtime()
    .with_grpc_server()
    .with_seed(ClosureSeed::new(|ctx| {
        let client = ctx.nats_client().clone();
        Box::pin(async move { publish_events(&client).await })
    }))
    .with_readiness_check("root-node-id", "expected-node-id")
    .build()
    .await?;

let mut client = fixture.query_client();
// ... gRPC calls ...
fixture.shutdown().await?;
```
