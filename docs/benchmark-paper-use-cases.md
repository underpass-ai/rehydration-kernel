# Running Paper Use Case Benchmarks

Run the paper artifact benchmarks with LLM-as-judge evaluation.

## Prerequisites

### 1. vLLM (local inference agent)

The vLLM service runs Qwen3-8B behind mTLS ingress at `llm.underpassai.com`.

Extract client certs from k8s:

```bash
kubectl get secret vllm-client-cert -n underpass-runtime \
  -o jsonpath='{.data.tls\.crt}' | base64 -d > /tmp/vllm-client.crt
kubectl get secret vllm-client-cert -n underpass-runtime \
  -o jsonpath='{.data.tls\.key}' | base64 -d > /tmp/vllm-client.key
```

Verify connectivity:

```bash
curl -sk --cert /tmp/vllm-client.crt --key /tmp/vllm-client.key \
  https://llm.underpassai.com/v1/models
```

### 2. API keys

API keys must never be hardcoded in scripts or committed to the repository.
Read them at runtime from a local file, password manager, or k8s secret.

```bash
# Anthropic (Claude) — read from local file
export ANTHROPIC_KEY="$(cat /path/to/anthropic-key.txt)"

# OpenAI (GPT-5.x) — read from local file
export OPENAI_KEY="$(cat /path/to/openai-key.txt)"

# Alternative: password manager
export ANTHROPIC_KEY="$(op read 'op://Vault/Anthropic/api-key')"

# Alternative: k8s secret
export ANTHROPIC_KEY="$(kubectl get secret anthropic-api \
  -n underpass-runtime -o jsonpath='{.data.api-key}' | base64 -d)"
```

### 3. Container runtime

Docker or Podman must be available for testcontainers (Neo4j, Valkey, NATS).

## Environment Variables

### Inference model (agent)

| Variable | Description |
|----------|-------------|
| `LLM_ENDPOINT` | Chat completions URL |
| `LLM_MODEL` | Model ID (e.g. `Qwen/Qwen3-8B`, `gpt-5.4`, `claude-opus-4-6-20250610`) |
| `LLM_PROVIDER` | `openai` (standard), `openai-new` (GPT-5.x/o3/o4), or `anthropic` |
| `LLM_API_KEY` | API key for the inference endpoint |
| `LLM_TEMPERATURE` | Sampling temperature (use `0.0` for deterministic runs) |
| `LLM_TLS_CERT_PATH` | mTLS client cert (vLLM only) |
| `LLM_TLS_KEY_PATH` | mTLS client key (vLLM only) |
| `LLM_TLS_INSECURE` | `true` to skip CA validation (self-signed, vLLM only) |

### Judge model

| Variable | Description |
|----------|-------------|
| `LLM_JUDGE_ENDPOINT` | Judge API URL |
| `LLM_JUDGE_MODEL` | Judge model ID |
| `LLM_JUDGE_PROVIDER` | `anthropic` or `openai-new` |
| `LLM_JUDGE_API_KEY` | API key for the judge endpoint |

### Output

| Variable | Description |
|----------|-------------|
| `PAPER_OUTPUT_DIR` | Override output directory (default: `artifacts/paper-use-cases`) |

## Evaluation Prompts

Prompts are in `crates/rehydration-testkit/resources/llm_prompts.yaml`.

- `inference_prompt` — sent to the agent with rendered context + question
- `judge_prompt` — sent to the judge with LLM response + ground truth

Override at runtime via `LLM_PROMPTS_PATH=/path/to/custom_prompts.yaml`.

The judge prompt is domain-aware: it distinguishes rationale preservation
(citing evidence from the context) from rationale inference (plausible but
not grounded in the rehydrated graph).

## Benchmark Configurations

### Config 1: Qwen3-8B (vLLM local) + Opus 4.6 judge

```bash
export LLM_ENDPOINT=https://llm.underpassai.com/v1/chat/completions
export LLM_MODEL=Qwen/Qwen3-8B
export LLM_PROVIDER=openai
export LLM_TLS_CERT_PATH=/tmp/vllm-client.crt
export LLM_TLS_KEY_PATH=/tmp/vllm-client.key
export LLM_TLS_INSECURE=true
export LLM_TEMPERATURE=0.0
export LLM_JUDGE_ENDPOINT=https://api.anthropic.com/v1/messages
export LLM_JUDGE_MODEL=claude-opus-4-6-20250610
export LLM_JUDGE_PROVIDER=anthropic
export LLM_JUDGE_API_KEY="$(cat /path/to/anthropic-key.txt)"

export PAPER_OUTPUT_DIR=artifacts/paper-use-cases-qwen3-8b-agent_opus46-judge
bash scripts/ci/integration-paper-use-cases.sh
```

### Config 2: GPT-5.4 + Opus 4.6 judge

```bash
export LLM_ENDPOINT=https://api.openai.com/v1/chat/completions
export LLM_MODEL=gpt-5.4
export LLM_PROVIDER=openai-new
export LLM_API_KEY="$(cat /path/to/openai-key.txt)"
export LLM_TEMPERATURE=0.0
export LLM_JUDGE_ENDPOINT=https://api.anthropic.com/v1/messages
export LLM_JUDGE_MODEL=claude-opus-4-6-20250610
export LLM_JUDGE_PROVIDER=anthropic
export LLM_JUDGE_API_KEY="$(cat /path/to/anthropic-key.txt)"

export PAPER_OUTPUT_DIR=artifacts/paper-use-cases-gpt54-agent_opus46-judge
bash scripts/ci/integration-paper-use-cases.sh
```

### Config 3: Opus 4.6 agent + GPT-5.4 judge (cross-validation)

```bash
export LLM_ENDPOINT=https://api.anthropic.com/v1/messages
export LLM_MODEL=claude-opus-4-6-20250610
export LLM_PROVIDER=anthropic
export LLM_API_KEY="$(cat /path/to/anthropic-key.txt)"
export LLM_TEMPERATURE=0.0
export LLM_JUDGE_ENDPOINT=https://api.openai.com/v1/chat/completions
export LLM_JUDGE_MODEL=gpt-5.4
export LLM_JUDGE_PROVIDER=openai-new
export LLM_JUDGE_API_KEY="$(cat /path/to/openai-key.txt)"

export PAPER_OUTPUT_DIR=artifacts/paper-use-cases-opus46-agent_gpt54-judge
bash scripts/ci/integration-paper-use-cases.sh
```

## Run All Configurations

```bash
# Run sequentially (~10 min each, ~30 min total)
for config in \
  "qwen3-8b-agent_opus46-judge" \
  "gpt54-agent_opus46-judge" \
  "opus46-agent_gpt54-judge"; do
  # Set env vars per config (see sections above)
  export PAPER_OUTPUT_DIR="artifacts/paper-use-cases-${config}"
  bash scripts/ci/integration-paper-use-cases.sh
done
```

## Output

Each run produces:

- `<output_dir>/cases/*.json` — per use-case metric files
- `<output_dir>/summary.json` — consolidated summary
- `<output_dir>/results.md` — rendered report
- `<output_dir>/results-figures.md` — figures

### Metric Fields

| Field | Description |
|-------|-------------|
| `query_latency_ms` | RPC round-trip time (ms) |
| `total_latency_ms` | End-to-end including LLM evaluation (ms) |
| `graph_load_ms` | Neo4j neighborhood load time (ms) |
| `detail_load_ms` | Valkey batch detail load time, MGET (ms) |
| `bundle_assembly_ms` | Bundle construction time (ms) |
| `detail_batch_size` | Number of nodes in batch detail load |
| `llm_task_success` | Judge: failure point correctly identified |
| `llm_restart_accuracy` | Judge: restart node correctly identified with causal justification |
| `llm_reason_preserved` | Judge: rationale preserved (not inferred) from context |
| `llm_latency_ms` | Combined inference + judge latency (ms) |

### Determinism

Use `LLM_TEMPERATURE=0.0` for reproducible results. Note that API-side
non-determinism (batching, quantization) may still cause minor variation
between runs.

## P1 Performance Measurement

Measures the impact of shared graph reads and batch detail loading
(multi-role vs repeated single-role):

```bash
bash scripts/ci/integration-kernel-full-journey.sh
```

The test prints a timing comparison table showing wall clock savings,
graph read savings, and detail load savings. Requires containers only
(no LLM endpoints).
