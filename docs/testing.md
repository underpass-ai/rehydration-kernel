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

Each evaluation calls an LLM for inference (agent) and a second LLM for judging.
The test uses testcontainers to spin up Neo4j + Valkey + NATS + kernel locally,
seeds data with the dataset generator, then runs gRPC queries and evaluates
the rendered context with the LLM pair.

### What runs where

```
Local machine                          External
┌────────────────────┐                 ┌──────────────────┐
│ testcontainers:    │                 │ vLLM (GPU)       │
│   Neo4j            │  agent call     │   Qwen3-8B       │
│   Valkey           │ ───────────────>│                  │
│   NATS             │                 └──────────────────┘
│   kernel (gRPC)    │                 ┌──────────────────┐
│                    │  judge call     │ OpenAI / Anthropic│
│ test binary        │ ───────────────>│   GPT-4.1-nano   │
│   (Rust)           │                 │   Claude Opus 4  │
└────────────────────┘                 └──────────────────┘
```

### Configuration

Matrix config: `crates/rehydration-testkit/resources/evaluation-matrix.yaml`
Judge prompts: `crates/rehydration-testkit/resources/llm_prompts.yaml`

### Tests

| Test | What it validates | Cost |
|:-----|:------------------|:-----|
| `llm_judge_prompt_evaluation` | Full matrix: agents x judges x scales x noise | ~$15 full, ~$0.10 diagnostic |
| `vllm_benchmark_integration` | vLLM inference quality under budget pressure | GPU time only |

### Environment variables

**Agent (inference):**

| Variable | Description |
|:---------|:------------|
| `LLM_ENDPOINT` | Inference endpoint (e.g. `https://llm.underpassai.com/v1/chat/completions`) |
| `LLM_MODEL` | Model name (e.g. `Qwen/Qwen3-8B`) |
| `LLM_PROVIDER` | `openai` (vLLM), `openai-new` (GPT-5.x), `anthropic` |
| `LLM_TEMPERATURE` | Sampling temperature (recommend `0.0` for reproducibility) |
| `LLM_TLS_CERT_PATH` | Client cert for mTLS endpoints (vLLM) |
| `LLM_TLS_KEY_PATH` | Client key for mTLS endpoints |
| `LLM_TLS_INSECURE` | Skip server cert verification (`true` for self-signed) |

**Judge:**

| Variable | Description |
|:---------|:------------|
| `LLM_JUDGE_ENDPOINT` | Judge endpoint (e.g. `https://api.openai.com/v1/chat/completions`) |
| `LLM_JUDGE_MODEL` | Judge model (e.g. `gpt-4.1-nano`, `claude-opus-4-6`) |
| `LLM_JUDGE_PROVIDER` | `openai-new`, `anthropic` |
| `LLM_JUDGE_API_KEY` | API key for the judge endpoint |

**Filters (subset the matrix):**

| Variable | Values |
|:---------|:-------|
| `FILTER_SCALES` | `micro`, `meso`, `stress` (comma or space separated) |
| `FILTER_NOISE` | `clean`, `competing`, `conflicting`, `restart` |

**API keys:**

| Variable | Source |
|:---------|:-------|
| `ANTHROPIC_KEY` | `cat /tmp/claude.txt` |
| `OPENAI_KEY` | `cat /tmp/openai.txt` |

### Run: cheapest possible (micro, clean, nano judge)

```bash
export OPENAI_KEY="$(cat /tmp/openai.txt)"

LLM_ENDPOINT="https://llm.underpassai.com/v1/chat/completions" \
LLM_MODEL="Qwen/Qwen3-8B" \
LLM_PROVIDER="openai" \
LLM_TLS_CERT_PATH="/tmp/vllm-client.crt" \
LLM_TLS_KEY_PATH="/tmp/vllm-client.key" \
LLM_TLS_INSECURE="true" \
LLM_TEMPERATURE="0.0" \
LLM_JUDGE_ENDPOINT="https://api.openai.com/v1/chat/completions" \
LLM_JUDGE_MODEL="gpt-4.1-nano" \
LLM_JUDGE_PROVIDER="openai-new" \
LLM_JUDGE_API_KEY="$OPENAI_KEY" \
FILTER_SCALES="micro" \
FILTER_NOISE="clean" \
bash -c '. scripts/ci/testcontainers-runtime.sh 2>/dev/null && \
cargo test -p rehydration-tests-paper \
  --features container-tests \
  --test llm_judge_prompt_evaluation \
  -- --nocapture --test-threads=1'
```

This runs 18 evaluations (2 domains x 3 mixes x 3 seeds) and costs ~$0.01.

### Run: ground truth diagnostic

```bash
export ANTHROPIC_KEY="$(cat /tmp/claude.txt)"
export OPENAI_KEY="$(cat /tmp/openai.txt)"
bash scripts/ci/e2e-ground-truth-diagnostic.sh
```

### Run: full matrix

```bash
bash scripts/ci/evaluate-prompt-variants.sh
```

### Viewing logs

The test writes to stdout with `--nocapture`. Key log lines to watch:

```
[PRECHECK]  — API connectivity and key validation (before any evals)
[CALIBRATE] — Judge calibration with known-good/known-bad responses
[CAPTURE]   — Per-variant: tokens, compression ratio, causal density
[EVAL]      — Agent inference + judge verdict per evaluation
[RESULT]    — Summary table with TaskOK / RestartOK / ReasonPreserved
```

If running in background, check the output file:

```bash
tail -f /path/to/output.log | grep -E '\[RESULT\]|\[EVAL\]|\[CAPTURE\]'
```

### Output artifacts

Results are saved to `artifacts/e2e-runs/<timestamp>/`:

```
artifacts/e2e-runs/<timestamp>/
├── results/           # One JSON per evaluation
│   ├── micro-ops-explanatory-seed0.json
│   ├── micro-ops-structural-seed0.json
│   └── ...
├── test.log           # Full test output
└── summary.json       # Aggregated results
```

Each result JSON contains:
- `rendered_tokens`, `raw_equivalent_tokens`, `compression_ratio`
- `causal_density`, `noise_ratio`, `detail_coverage`
- `agent_response`, `judge_raw`
- `task_correct`, `restart_exact`, `reason_correct`

These metrics come from the **kernel response** (`rendered.quality` in the
proto), not computed by the test. Single source of truth.

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
