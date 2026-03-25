# Rehydration Kernel

Node-centric context rehydration for agentic systems.

## What This Repo Is

`rehydration-kernel` is a generic context engine built around four public
concepts:

- root node
- neighbor nodes
- relationships
- extended node detail

The kernel does not own product-specific nouns. Integrating products are
expected to map their own domain language to this graph model at the edge.
The kernel also assumes its own infrastructure dependencies are present:
Neo4j, Valkey, and NATS are required runtime components, not optional features.

## Current Status

This repo is functionally complete for the kernel-owned migration scope.
Known limitations on reserved query fields are documented in
[`docs/beta-status.md`](./docs/beta-status.md).

What is already in place:

- graph-native domain and application layers
- split Neo4j, Valkey, NATS, and gRPC adapters
- frozen node-centric gRPC and async contracts
- contract CI with `buf breaking`, AsyncAPI checks, and boundary naming policy
- container-backed integration tests
- full kernel journey end-to-end coverage:
  - projection -> query -> compatibility -> command -> admin
  - full TLS variant across gRPC, NATS, Valkey, and Neo4j in the test harness
- agentic end-to-end proofs:
  - pull-driven runtime flow against a narrow runtime contract shape
  - event-driven runtime trigger flow
- cluster-verifiable starship journey demo for a production-like graph case
- runtime integration reference docs and runnable client example

What is intentionally out of scope for this repo:

- `swe-ai-fleet` legacy noun modeling
- `planning.*` or `orchestration.*` consumers
- product-side shadow mode implementation
- rollout and rollback logic

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        gRPC (mTLS)                               │
│  GetContext · GetContextPath · RehydrateSession · UpdateContext   │
└────────────────────────────┬─────────────────────────────────────┘
                             │
┌────────────────────────────▼─────────────────────────────────────┐
│                    Application Layer                              │
│  Query: rehydrate → render → truncate (cl100k_base, salience)    │
│  Command: validate → append event (optimistic concurrency)       │
└──────┬──────────────┬──────────────┬──────────────┬──────────────┘
       │              │              │              │
┌──────▼──────┐ ┌─────▼─────┐ ┌─────▼─────┐ ┌─────▼──────┐
│   Neo4j     │ │  Valkey   │ │   NATS    │ │   OTLP     │
│  (graph)    │ │ (snapshot │ │ (events + │ │  (traces + │
│             │ │  + detail)│ │  commands)│ │   metrics) │
└─────────────┘ └───────────┘ └───────────┘ └────────────┘
       all connections mTLS · Helm-managed · sidecar-capable
```

Non-negotiable rules: DDD first, hexagonal boundaries, one concept per file,
one use case per file.

**Infrastructure stack:**

- **Neo4j** — graph state: nodes, relationships, traversal
- **Valkey** — snapshots, node detail, event store (RESP protocol)
- **NATS JetStream** — projection events, command event store (optimistic concurrency)
- **gRPC + mTLS** — all external and internal boundaries TLS-encrypted
- **OpenTelemetry** — traces and metrics via OTLP (7 instruments)
- **Helm chart** — production-ready with optional NATS/Valkey sidecars, TLS config, OTel endpoint
- **cl100k_base** — BPE tokenization (tiktoken-rs) for accurate token budgets
- **Salience ordering** — causal > motivational > evidential > constraint > procedural > structural

## Contracts

Primary public artifacts:

- gRPC proto:
  - [`api/proto/underpass/rehydration/kernel/v1beta1`](./api/proto/underpass/rehydration/kernel/v1beta1)
- async contract:
  - [`api/asyncapi/context-projection.v1beta1.yaml`](./api/asyncapi/context-projection.v1beta1.yaml)
- contract examples:
  - [`api/examples/README.md`](./api/examples/README.md)
- runtime integration reference:
  - [`docs/migration/kernel-runtime-integration-reference.md`](./docs/migration/kernel-runtime-integration-reference.md)

Historical migration and handoff docs live under [`docs/migration`](./docs/migration).
Many of them are historical after the `v1beta1` cut and compatibility removal.
Use the active integration references first, and treat older migration notes as
review-required unless they explicitly say otherwise.

## Repo Layout

- `api/proto`: gRPC contracts
- `api/asyncapi`: async contracts
- `api/examples`: canonical request, response, and event fixtures
- `crates/rehydration-domain`: domain model and invariants
- `crates/rehydration-ports`: application-facing ports
- `crates/rehydration-application`: use cases and orchestration
- `crates/rehydration-adapter-*`: infrastructure adapters
- `crates/rehydration-transport-*`: transport boundaries
- `crates/rehydration-server`: composition root
- `crates/rehydration-testkit`: testing helpers
- `scripts/ci`: local quality and integration gates
- `docs/migration`: closeout, handoff, and integration strategy docs

## Benchmark

LLM-as-judge evaluation across 18 configurations (3 scales × 2 domains × 3
relation mixes). Each cell is `TaskOK + RestartOK + ReasonPreserved` out of 3.

### What we measured

| Model | Budget | Explanatory | Structural | Gap |
|-------|:------:|:----------:|:----------:|:---:|
| GPT-5.4 (frontier) | 4096 | 17/18 (94%) | 11/18 (61%) | 33pp |
| Claude Opus 4 (frontier) | 4096 | 15/18 (83%) | 10/18 (56%) | 27pp |
| Qwen3-8B (local 8B) | 4096 | 18/18 (100%) | 9/18 (50%) | 50pp |
| Qwen3-8B (local 8B) | **512** | **15/18 (83%)** | 8/18 (44%) | 39pp |
| Qwen3-8B + ResumeFocused | **512** | **16/18 (89%)** | 5/18 (28%) | 61pp |
| Qwen3-8B + competing noise | 4096 | **18/18 (100%)** | 5/18 (28%) | **72pp** |

**What this shows:**

- Explanatory relationships consistently outperform structural-only across
  all models, budgets, and noise conditions. The gap ranges from 27pp to 72pp.
- The 100% score from Qwen3-8B at full budget reflects extraction, not
  reasoning — rationale text is present verbatim and the model copies it.
- `ResumeFocused` mode auto-activates under token pressure (< 30 tokens/node)
  and prunes distractor branches, keeping only the causal spine. This fixed
  `stress-ops-explanatory` from 0/3 to 3/3 at 512-token budget.
  Under token pressure (512 tokens, 68-85% truncation), accuracy drops to 83%.
- Structural-only accuracy degrades with model size (61% → 56% → 50%),
  suggesting weaker models benefit more from explanatory metadata.
- With competing causal noise (distractors that mimic real domain nodes with
  causal semantic classes), explanatory stays at 100% while structural drops
  to 28%. The kernel's explanatory metadata is immune to plausible-looking
  noise; without it, the model cannot distinguish real from competing paths.

**What this does NOT show:**

- These are synthetic graphs, not production workloads.
- The evaluation uses a single judge model per run — results may vary with
  different judges.
- The benchmark measures context quality, not end-to-end agent task completion.
- We do not claim this system outperforms GraphRAG, HippoRAG, or other
  retrieval systems — the focus is on bounded, explanatory context for
  agent continuity, not general-purpose QA.

### Full results by configuration

**GPT-5.4 inference + Claude Opus 4 judge (budget 4096):**

| Config | Explanatory | Structural | Mixed |
|--------|:-----------:|:----------:|:-----:|
| micro-ops | 3/3 | 2/3 | 3/3 |
| micro-debug | 3/3 | 2/3 | 3/3 |
| meso-ops | 2/3 | 2/3 | 3/3 |
| meso-debug | 3/3 | 2/3 | 3/3 |
| stress-ops | 3/3 | 2/3 | 3/3 |
| stress-debug | 3/3 | 1/3 | 3/3 |

**Qwen3-8B vLLM + Claude Opus 4 judge (budget 4096):**

| Config | Explanatory | Structural | Mixed |
|--------|:-----------:|:----------:|:-----:|
| micro-ops | 3/3 | 2/3 | 3/3 |
| micro-debug | 3/3 | 0/3 | 3/3 |
| meso-ops | 3/3 | 2/3 | 3/3 |
| meso-debug | 3/3 | 2/3 | 3/3 |
| stress-ops | 3/3 | 2/3 | 3/3 |
| stress-debug | 3/3 | 1/3 | 3/3 |

**Qwen3-8B vLLM + Claude Opus 4 judge (budget 512 — truncated):**

| Config | Explanatory | Structural | Mixed | Tokens |
|--------|:-----------:|:----------:|:-----:|:------:|
| micro-ops | 3/3 | 1/3 | 3/3 | 354 |
| micro-debug | 3/3 | 1/3 | 3/3 | 340 |
| meso-ops | 3/3 | 2/3 | 3/3 | ~490 |
| meso-debug | 3/3 | 2/3 | 2/3 | ~494 |
| stress-ops | 0/3 | 2/3 | 3/3 | ~506 |
| stress-debug | 3/3 | 0/3 | 3/3 | ~492 |

Under 512-token budget, meso explanatory holds at 3/3 (68% truncated) while
stress-ops-explanatory drops to 0/3 (85% truncated, critical context lost).
The kernel's salience ordering (causal > motivational > evidential > structural)
preserves the most important relationships under pressure.

Scales: micro (4 nodes), meso (21 nodes), stress (49 nodes).
Domains: operations (incident response), software debugging (hypothesis/fix cycle).

Evaluation prompts are externalized to
[`crates/rehydration-testkit/resources/llm_prompts.yaml`](./crates/rehydration-testkit/resources/llm_prompts.yaml)
and can be overridden at runtime via `LLM_PROMPTS_PATH`.

## Paper Artifact

The repository also contains an artifact-backed paper draft on explanatory
graph context rehydration for agentic systems.

Current paper materials:

- submission draft:
  [`docs/PAPER_SUBMISSION_DRAFT.md`](./docs/PAPER_SUBMISSION_DRAFT.md)
- ACL package index:
  [`docs/paper/README.md`](./docs/paper/README.md)
- ACL LaTeX sources:
  [`docs/paper/acl/`](./docs/paper/acl/) (build locally with `pdflatex`)
- paper-use-case artifact summary:
  [`artifacts/paper-use-cases/summary.json`](./artifacts/paper-use-cases/summary.json)
- paper-use-case rendered report:
  [`artifacts/paper-use-cases/results.md`](./artifacts/paper-use-cases/results.md)

The current manuscript evaluates four use cases:

- failure diagnosis with rehydration-point recovery
- why a task was implemented in a particular way
- interrupted handoff with resumable execution
- constraint-preserving retrieval under token pressure

## Quickstart

Toolchain:

- Rust `1.90.0`, pinned in [`rust-toolchain.toml`](./rust-toolchain.toml)

Core checks:

```bash
cargo fmt --all
cargo check --workspace
cargo test --workspace
```

Repository gate:

```bash
bash scripts/ci/quality-gate.sh
```

Focused contract gate:

```bash
bash scripts/ci/contract-gate.sh
```

Container-backed integration targets:

```bash
bash scripts/ci/integration-valkey.sh
bash scripts/ci/integration-neo4j.sh
bash scripts/ci/integration-agentic-context.sh
bash scripts/ci/integration-agentic-event-context.sh
bash scripts/ci/integration-kernel-full-journey.sh
bash scripts/ci/integration-kernel-full-journey-tls.sh
```

For deployed kernels, the generic projection runtime persists its own state in
Valkey through `REHYDRATION_RUNTIME_STATE_URI`.

Container image build check:

```bash
bash scripts/ci/container-image.sh
```

The script uses `docker` when available and falls back to `podman`. Override
with `CONTAINER_RUNTIME=docker` or `CONTAINER_RUNTIME=podman` if you need to
force one runtime.

Helm chart lint:

```bash
bash scripts/ci/helm-lint.sh
```

## Public Readiness Notes

If you are integrating another product with this kernel, start here:

- [`docs/migration/kernel-node-centric-integration-contract.md`](./docs/migration/kernel-node-centric-integration-contract.md)
- [`docs/migration/kernel-runtime-integration-reference.md`](./docs/migration/kernel-runtime-integration-reference.md)
- [`docs/migration/kernel-repo-closeout.md`](./docs/migration/kernel-repo-closeout.md)

If you are integrating `swe-ai-fleet`, the handoff docs are:

- [`docs/migration/swe-ai-fleet-node-centric-integration-strategy.md`](./docs/migration/swe-ai-fleet-node-centric-integration-strategy.md)
- [`docs/migration/swe-ai-fleet-shadow-mode-spec.md`](./docs/migration/swe-ai-fleet-shadow-mode-spec.md)
- [`docs/migration/swe-ai-fleet-integration-checklist.md`](./docs/migration/swe-ai-fleet-integration-checklist.md)

## Runtime And Deployment Ecosystem

This repo owns the kernel code, contracts, and integration proofs.

Operational packaging may live in sibling repos that consume the kernel. In the
current ecosystem that includes a sibling runtime capable of:

- publishing container artifacts to GitHub Container Registry
- running under Docker Compose
- running on Kubernetes through Helm

That deployment packaging is intentionally kept outside the kernel repo when it
belongs to the runtime or product layer rather than to the kernel itself.

See:

- [`docs/operations/README.md`](./docs/operations/README.md)
- [`docs/operations/deployment-boundary.md`](./docs/operations/deployment-boundary.md)
- [`docs/operations/container-image.md`](./docs/operations/container-image.md)

## Standalone Container Image

The kernel now owns a standalone OCI image intended for external download and
evaluation.

Public location:

- `ghcr.io/underpass-ai/rehydration-kernel`

Typical pull:

```bash
docker pull ghcr.io/underpass-ai/rehydration-kernel:latest
```

See [`docs/operations/container-image.md`](./docs/operations/container-image.md)
for environment variables, tags, and usage.

Helm chart:

- source chart: [`charts/rehydration-kernel`](./charts/rehydration-kernel)
- OCI location: `oci://ghcr.io/underpass-ai/charts/rehydration-kernel`

The default chart values are intentionally secure:

- no implicit `latest` image tag
- no inline backend URIs by default
- production-style installs should use `image.digest` or a pinned tag plus `secrets.existingSecret`
- optional `ingress.enabled` can expose the gRPC service through a controller-managed ingress
- optional `neo4jTls.*` can mount a custom Neo4j CA for secure `graphUri` values

The sibling-runtime deployment profiles are:

- [`charts/rehydration-kernel/values.underpass-runtime.yaml`](./charts/rehydration-kernel/values.underpass-runtime.yaml) for the current cluster wiring, including the NGINX gRPC ingress host `rehydration-kernel.underpassai.com`
- [`charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml`](./charts/rehydration-kernel/values.underpass-runtime.secure.example.yaml) for the staged Neo4j TLS target once the shared graph service publishes a CA-backed TLS endpoint

For local evaluation only, use [`values.dev.yaml`](./charts/rehydration-kernel/values.dev.yaml).

## License

Apache-2.0. See [`LICENSE`](./LICENSE).
