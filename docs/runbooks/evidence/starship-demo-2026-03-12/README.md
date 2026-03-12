# Starship Demo Evidence 2026-03-12

This folder stores the evidence captured from the first successful in-cluster
`Repair The Starship` demo run against real infrastructure.

Classification:

- this is a product demo harness
- this is not a PR-gated end-to-end test
- its purpose is to demonstrate the node-centric flow and expose integration
  weaknesses that should later be hardened

Execution context:

- date: `2026-03-12T02:21:36Z`
- namespace: `underpass-runtime`
- job: `starship-demo-1773281681`
- demo job image: `ghcr.io/underpass-ai/rehydration-kernel:starship-demo-20260312-020201`
- kernel endpoint: `http://rehydration-kernel:50054`
- NATS endpoint: `nats://nats:4222`
- LLM provider: `vllm`
- LLM endpoint: `http://vllm-server:8000`
- model: `Qwen/Qwen3-8B`
- runtime mode: `filesystem`

Artifacts:

- [`job.yaml`](./job.yaml)
- [`job.log`](./job.log)
- [`summary.json`](./summary.json)
- [`services.txt`](./services.txt)
- [`neo4j-nodes.csv`](./neo4j-nodes.csv)
- [`neo4j-relationships.csv`](./neo4j-relationships.csv)
- [`valkey-checks.txt`](./valkey-checks.txt)
- [`vllm-probe-simple.json`](./vllm-probe-simple.json)
- [`vllm-probe-selection.json`](./vllm-probe-selection.json)

What the successful run proves:

- the demo works over internal cluster DNS only; no `port-forward` was used
- `rehydration-kernel` consumed projection events from NATS and materialized
  graph state in Neo4j
- a real model endpoint selected the current step, generated deliverables, and
  resumed after rehydration
- the resumed phase wrote only phase-2 deliverables:
  - `src/commands/route.rs`
  - `src/commands/status.rs`
  - `tests/starship_cli.rs`
  - `captains-log.md`

Direct persistence verification captured after the run:

- Neo4j: confirmed
  - run-scoped mission, dependency, and work-item nodes were found
  - expected `contains` and `depends_on` relationships were found
- Valkey: not confirmed
  - `DBSIZE` returned `0`
  - no `rehydration:*` keys were present
  - explicit `GET` checks for node details and projection checkpoints returned
    `nil`

Operational interpretation of the Valkey result:

- the demo `Job` ran image `ghcr.io/underpass-ai/rehydration-kernel:starship-demo-20260312-020201`
- the live kernel deployment queried during follow-up was still running
  `ghcr.io/underpass-ai/rehydration-kernel:starship-demo-20260312-015959`
- because of that mismatch, Neo4j persistence is proven for this run but the
  Valkey path remains unproven and must be rerun after redeploying the kernel
  with the newer image

Observed weaknesses surfaced by the demo:

- the raw step-selection prompt could produce an empty JSON object with this
  vLLM model
- large single-shot file plans could exceed the robustness of the JSON parser
- the demo therefore drove two improvements:
  - tolerate transient `Node not found` while projections materialize
  - generate deliverables one file at a time, with repair retry on invalid JSON

Interpretation rule:

- these artifacts are evidence for product behavior and integration maturity
- they are not release-gate evidence in the same sense as CI-backed integration
  tests

LLM parser contrast with the copied `swe-ai-fleet` implementation:

- `json_response_parser.py`
  - extracts JSON from markdown fences
  - then calls `json.loads(...)`
  - does not strip `<think>...</think>`
  - does not retry or repair malformed JSON
- `vllm_client_adapter.py`
  - returns `choices[0].message.content.strip()`
  - does not inspect `reasoning` or `reasoning_content`
  - does not reject empty JSON objects like `{}`
- `generate_plan_usecase.py`
  - parses once
  - only checks that `"steps"` exists
  - has no recovery path when the first JSON payload is weak or malformed

Required follow-up for the next session:

- redeploy the kernel with the newer image and rerun the demo so Valkey can be
  verified against the same code path exercised by the Job
- recheck Valkey directly and confirm that node details, processed events, and
  projection checkpoint data are written during the rerun
- compare the observed `vllm` response patterns here against the copied
  `swe-ai-fleet` LLM response handling in:
  - `core/agents_and_tools/agents/infrastructure/services/json_response_parser.py`
  - `core/agents_and_tools/agents/infrastructure/adapters/vllm_client_adapter.py`
  - `core/agents_and_tools/agents/application/usecases/generate_plan_usecase.py`

That follow-up matters because this demo should validate the full persistence
path, and because the future integrating product must handle the same response
shapes safely, not only produce plausible files in this demo.
