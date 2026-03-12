# Starship Rehydration Demo

## Goal

Run the `Repair The Starship` scenario as a reproducible demo of
stepwise rehydration.

Important classification:

- this flow is a demo harness
- this is not a PR-gated verification test
- we keep it to show the product story, reveal failure modes, and drive
  improvements in the real integration path

The scenario proves that:

- an agent completes phase 1 of a mission
- the graph context changes while the agent is offline
- a fresh agent rehydrates from the kernel
- the new agent continues phase 2 without overwriting phase 1 deliverables

## What The Demo Produces

Phase 1 writes:

- `src/commands/scan.rs`
- `src/commands/repair.rs`
- `state/starship-state.json`

Phase 2 resumes and writes:

- `src/commands/route.rs`
- `src/commands/status.rs`
- `tests/starship_cli.rs`
- `captains-log.md`

## Demo Modes

There are two useful modes:

- deterministic smoke/demo harness
- real external-LLM manual harness

The deterministic mode is good for CI and repeatability.

The real-LLM mode is the one that matches the intended product story: the actor
deciding the continuation is an actual external model endpoint.

For that reason, the real-LLM Starship flow should be treated as:

- a demonstration of behavior
- a debugging and discovery harness
- a source of evidence for integration maturity

It should not be treated as a hard release gate.

## Run The Demo

Normal run:

```bash
bash scripts/demo/run-starship-demo-smoke.sh
```

Debug run with persistent log:

```bash
bash scripts/demo/run-starship-demo-debug.sh /tmp/starship-rehydration-debug.log
```

If no path is provided, the debug script writes to:

```text
/tmp/starship-rehydration-debug.log
```

## Run With A Real LLM

Supported providers:

- `vllm`
- `openai`
- `anthropic`
- `openai_compat`

### vLLM

```bash
export LLM_PROVIDER=vllm
export VLLM_BASE_URL=http://127.0.0.1:8000
export VLLM_MODEL=<your-model-name>
```

Optional:

```bash
export VLLM_API_KEY=<token-if-required>
```

### OpenAI

```bash
export LLM_PROVIDER=openai
export OPENAI_API_KEY=<your-openai-key>
export OPENAI_MODEL=gpt-4.1-mini
```

Optional:

```bash
export OPENAI_BASE_URL=https://api.openai.com
```

### Anthropic / Claude

```bash
export LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY=<your-anthropic-key>
export ANTHROPIC_MODEL=claude-3-7-sonnet-latest
```

Optional custom gateway:

```bash
export ANTHROPIC_BASE_URL=https://api.anthropic.com
```

### OpenAI-compatible custom gateway

```bash
export LLM_PROVIDER=openai_compat
export OPENAI_COMPAT_BASE_URL=http://127.0.0.1:8000
export OPENAI_MODEL=<your-model-name>
```

Optional:

```bash
export OPENAI_API_KEY=<token-if-required>
```

Manual execution:

```bash
bash scripts/demo/run-starship-demo-real-llm.sh
```

This path runs the ignored demo target:

- `starship_real_llm_demo`

It is intentionally not part of default CI because it depends on a live model
server.

## Kubernetes And Compose Network Model

The demo is designed to avoid `port-forward`.

Kubernetes:

- run it as a `Job`
- use internal service DNS such as:
  - `rehydration-kernel:50054`
  - `nats:4222`
  - `vllm-server:8000`

Compose:

- run it as a service on the same compose network
- use service names, not host-mapped ports

Reference launchers:

- [`../../scripts/demo/run-starship-demo-k8s-job.sh`](../../scripts/demo/run-starship-demo-k8s-job.sh)
- [`../../scripts/demo/run-starship-demo-compose.sh`](../../scripts/demo/run-starship-demo-compose.sh)

## Captured Evidence

The first successful in-cluster demo run is stored here:

- [`evidence/starship-demo-2026-03-12/README.md`](./evidence/starship-demo-2026-03-12/README.md)

That evidence bundle includes:

- the exact Job manifest
- the raw Job log
- the final JSON summary
- cluster service snapshot
- direct Neo4j verification snapshots
- direct Valkey verification snapshots
- raw `vllm` probes that explain the model-behavior hardening we had to add

## Store Verification Status

Follow-up verification against the real backing stores produced a split result:

- Neo4j: confirmed
  - mission root node found
  - both work-item nodes found
  - expected `contains` and `depends_on` relationships found
- Valkey: not confirmed in the deployed kernel image that was queried
  - `DBSIZE` returned `0`
  - no `rehydration:*` keys were present
  - explicit node-detail and checkpoint reads returned `nil`

Important operational caveat:

- the demo `Job` used image tag `starship-demo-20260312-020201`
- the live kernel deployment queried during verification still ran
  `starship-demo-20260312-015959`
- that means Neo4j persistence is demonstrated for the successful run, but the
  Valkey path still needs a rerun after redeploying the kernel with the newer
  image

The goal remains to prove not only that `vllm` responded coherently, but that
the kernel persisted and resumed the node-centric context correctly through the
full real infrastructure path.

## `swe-ai-fleet` Parser Contrast

The next integrating product must not assume a simpler LLM response model than
the one exposed by this demo.

Concrete reference points in the copied `swe-ai-fleet` tree:

- `core/agents_and_tools/agents/infrastructure/services/json_response_parser.py`
- `core/agents_and_tools/agents/infrastructure/adapters/vllm_client_adapter.py`
- `core/agents_and_tools/agents/application/usecases/generate_plan_usecase.py`

What to verify there:

- whether wrapped markdown-only parsing is still assumed
- whether empty JSON objects like `{}` are treated as valid enough
- whether `<think>...</think>` or provider-specific reasoning fields would be ignored
- whether retry or repair logic exists when the first JSON payload is malformed

Current observed answer from the copied code:

- markdown fences are handled
- `<think>...</think>` is not stripped
- there is no repair retry after malformed JSON
- the raw `message.content` string from vLLM is passed straight into JSON parsing

## Remaining Follow-up

- redeploy the kernel with the newer image used by the successful demo Job
- rerun the demo and repeat the Valkey verification
- only then treat the full persistence path as demonstrated

## What To Look For In The Log

Useful log lines include:

- fixture startup and container endpoints
- published `starship` projection events
- current step selection before and after rehydration
- runtime `fs.write` operations
- the absence of phase 1 rewrites during phase 2

Expected milestones:

1. phase 1 selects `node:work_item:stabilize-sensors-and-hull`
2. resume events mark phase 1 as completed and phase 2 as in progress
3. phase 2 selects `node:work_item:plot-route-and-report-status`
4. `captains-log.md` is written with the mission title and resumed context

## Runtime Modes Covered

The underlying test target exercises two runtime modes:

- `RecordingRuntime`
- HTTP contract mode compatible with the reference runtime shape

Both deterministic runtime modes are executed by the same smoke/demo command
because the test binary contains two test cases.
