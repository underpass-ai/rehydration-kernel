# Operator Test Architecture

## Decision

Operator training, synthetic data generation, evaluation, and replay are product
surfaces in their own right. They are not loose scripts around the kernel.

The Operator code is now split into bounded contexts. Each bounded context uses
its own hexagonal shape:

```text
underpass-operator-<context>-domain
underpass-operator-<context>-application
underpass-operator-<context>-infra
```

The shared contract is also a bounded context, not a dumping ground:

```text
crates/underpass-operator-shared-domain
crates/underpass-operator-shared-contract
crates/underpass-operator-shared-application
crates/underpass-operator-shared-infra
```

Current bounded contexts:

```text
shared      -> common Operator vocabulary and trajectory contract
synthetic   -> synthetic training-data creation
evaluation  -> prediction scoring and evaluation metrics
training    -> training run planning and readiness metrics
replay      -> live MCP replay of Operator predictions
```

Benchmark exporters are intentionally not listed as Operator bounded contexts.
MemoryArena, LongMemEval, MemoryAgentBench, or any future benchmark can provide
external adapters, but those adapters must not define Operator language. They
may translate benchmark artifacts into the shared trajectory contract when that
is useful for an experiment.

The canonical product term is **trajectory builder**, not exporter.

```text
trajectory builder -> constructs Operator trajectories from KMP/MCP use-case specs
benchmark adapter  -> translates external benchmark artifacts when needed
exporter           -> reserved for external artifact extraction, not Operator core
```

This distinction is intentional. Operator training should be shaped by KMP/MCP
capabilities and use cases, not by benchmark file formats.

More bounded contexts can be added later by following the same pattern. They
must not be added as modules inside an existing context unless the language is
truly shared.

## Dependency Rule

Operator domain, application, contract, and infra crates are intentionally
independent from the kernel implementation. They must not depend on any
`rehydration-*` crate.

`rehydration-testkit` is also outside Operator. It may keep benchmark and kernel
test utilities, but it must not contain Operator binaries, Operator facades, or
dependencies on `underpass-operator-*` crates. Operator CLIs live in dedicated
Operator CLI crates:

```text
underpass-operator-synthetic-cli           -> KMP/MCP conformance trajectory builder
underpass-operator-evaluation-cli          -> policy eval, contract coverage, LLM baseline
underpass-operator-benchmark-adapters-cli  -> external benchmark trajectory adapters
underpass-operator-replay-cli              -> live MCP replay composition root
```

Executable composition roots are the exception when they must wire the current
kernel adapter. Today that exception is explicit and narrow:

```text
underpass-operator-replay-cli -> rehydration-mcp
```

The replay CLI owns process wiring only. Replay decisions, rows, metrics,
JSONL reading, and report writing live in replay domain/application/infra.

Allowed direction:

```text
shared-domain
  ^
  |
<bounded>-domain
  ^
  |
<bounded>-application
  ^
  |
<bounded>-infra
```

The shared context may have contract/application/infra around shared DTOs and
trajectory mapping. A bounded context may depend on `shared-domain`, but bounded
contexts must not depend on each other.

This is valid:

```text
underpass-operator-evaluation-domain -> underpass-operator-shared-domain
```

This is not valid:

```text
underpass-operator-training-domain -> underpass-operator-evaluation-domain
underpass-operator-synthetic-infra -> rehydration-domain
```

The no-kernel-dependency rule is enforced by unit tests named:

```text
crate_has_no_rehydration_dependencies
```

## Shared Context

The shared context contains only language that is genuinely common across
Operator bounded contexts:

- `KmpMcpCapability`;
- `KernelTool`;
- `OperatorMode`;
- `AllowedTools`;
- `OperatorAction`;
- `PreparedPayloadSource`;
- `AnswerPolicy`;
- `TrainingTrajectory`;
- shared ids and refs;
- shared trajectory DTOs;
- shared JSON/DTO/domain mappers.

The shared context does not own:

- synthetic data generation policy;
- evaluation scoring semantics;
- training run policy;
- benchmark-specific logic;
- model training code;
- MCP/gRPC execution;
- kernel persistence or kernel application services.

Benchmark-specific exporters must stay outside the Operator core. They are
experiment adapters, not product architecture.

## Metrics Rule

Metrics belong in domain.

Application use cases may collect inputs, call ports, and orchestrate a flow.
They must not become the owner of metrics semantics.

Infra may serialize, persist, or render reports. It must not decide what a
metric means.

Current domain-owned metrics:

| Context | Domain metric/report |
| --- | --- |
| `shared-domain` | `TrainingDatasetPreflightReport` |
| `synthetic-domain` | `SyntheticCaseGenerationMetric`, `SyntheticDatasetGenerationReport` |
| `evaluation-domain` | `EvaluationReport`, `EvaluationMetrics`, `ContractEvaluationCoverageReport` |
| `training-domain` | `TrainingRunMetrics`, `TrainingRunReadinessReport`, `TrainingContractCoverageReport` |
| `replay-domain` | `ReplaySummary`, `ReplayCounters`, `ActionLatencySummary` |

This rule matters because the model, evaluator, trajectory builder, and replay
runner must all speak the same metric language. If a metric lives in a binary
or a script, it will drift.

## Tell, Do Not Ask

Use cases should not pull primitive fields out of domain objects and recreate
domain decisions in application code.

Preferred shape:

```text
application -> reads from a port
application -> passes typed data into domain
domain      -> validates, classifies, calculates metrics, builds report
```

Current examples:

- `TrainingDatasetPreflightReport::from_trajectories(...)` owns duplicate
  `step_id` detection and dataset preflight metrics.
- `SyntheticCaseSpec::generation_metric(...)` owns the minimum generated sample
  rule for a synthetic case.
- `SyntheticDatasetBlueprint::for_kmp_mcp_capabilities(...)` owns the mapping
  from contract capability to synthetic case spec.
- `EvaluationReport::from_cases(...)` owns prediction scoring and evaluation
  metrics.
- `ContractEvaluationCoverageReport::from_cases(...)` owns evaluation coverage
  against the KMP/MCP contract inventory.
- `TrainingContractCoverageReport::from_trajectories(...)` owns training
  coverage against the KMP/MCP contract inventory.
- `TrainingRunReadinessReport::new(...)` owns training readiness metrics.
- `build_replay_summary(...)` owns replay counters, action latency metrics,
  missing-ref metrics, and replay metadata.

Do not force this rule where it only adds indirection. If a use case is simply
calling a port and returning a domain object, keep it simple.

## KMP/MCP Contract Coverage Rule

Every KMP/MCP contract capability must be visible in the active Operator
bounded contexts as a use case.

The shared domain owns the canonical inventory:

```text
KmpMcpCapability::all()
```

The bounded contexts consume that inventory. They do not recreate local tool
lists.

| Bounded context | Use case | Responsibility |
| --- | --- | --- |
| Synthetic creation | `BuildOperatorTrajectoryUseCase` | Build one canonical Operator trajectory from a KMP/MCP case spec. |
| Synthetic creation | `PlanKmpMcpSyntheticCasesUseCase` | Build one synthetic case plan per KMP/MCP capability. |
| Evaluation | `EvaluateKmpMcpContractCoverageUseCase` | Score predictions and report missing/covered contract capabilities. |
| Training | `AssessKmpMcpTrainingContractCoverageUseCase` | Check whether training trajectories cover each contract capability. |
| Replay | `ReplayMcpPredictionsUseCase` | Execute validated predictions against live MCP through a tool-caller port. |

When a new KMP/MCP capability is added, the change starts in the shared
inventory. The bounded contexts must then expose it through their use
cases. If the capability is not trainable or not evaluable yet, that should be
an explicit missing capability in a domain report, not a silent omission.

## Context Boundaries

### Shared

Domain:

- shared value objects;
- KMP/MCP tool vocabulary;
- action and trajectory invariants;
- shared report types that are not specific to one bounded context.

Contract:

- serializable DTOs only;
- no domain decisions;
- no scoring;
- no validation beyond serde shape.

Application:

- ports and use cases over shared trajectories;
- dataset preflight orchestration.

Infra:

- JSON -> DTO;
- DTO -> domain;
- domain -> DTO;
- DTO -> JSON;
- in-memory adapters used by unit tests.

### Synthetic Creation

Domain:

- `SyntheticCaseSpec`;
- `SyntheticDatasetBlueprint`;
- `SyntheticDataset`;
- trajectory-builder vocabulary for constructing canonical Operator
  trajectories from KMP/MCP use-case specs;
- contract capability blueprint planning;
- generation metrics and generation report.

Application:

- `BuildOperatorTrajectoryUseCase`;
- `SyntheticCaseGenerator` port;
- `PlanKmpMcpSyntheticCasesUseCase`;
- `GenerateSyntheticDatasetUseCase`;
- fail-fast orchestration when a case underproduces examples.

Infra:

- adapters for concrete generation sources;
- `TeacherSyntheticCaseGenerator`, which receives raw teacher candidate rows
  and maps them through the shared trajectory mapper before they can enter the
  dataset;
- current in-memory adapter for unit tests.

### Evaluation

Domain:

- `EvaluationCase`;
- `EvaluationOutcome`;
- `EvaluationVerdict`;
- `EvaluationReport`;
- `EvaluationMetrics`;
- `ContractEvaluationCoverageReport`;
- `PolicyEvaluator`, `PolicyEvalRequest`, `PolicyEvalResult` and policy
  scoring metrics.

Application:

- `EvaluationCaseReader` port;
- `EvaluateKmpMcpContractCoverageUseCase`;
- `EvaluateOperatorPredictionsUseCase`;
- `EvaluateOperatorPolicyUseCase`.

Infra:

- adapters that read evaluation cases, policy-eval JSONL, or write reports;
- `JsonlValueReader`, which owns generic JSONL reading for evaluation CLIs;
- `JsonlPolicyEvalReader`, which reads raw trajectory JSONL and model-facing
  eval JSONL into `PolicyEvalTrajectory`;
- current in-memory reader for unit tests.

### Training

Domain:

- `TrainingDatasetManifest`;
- `TrainingRunPlan`;
- `TrainingRunMetrics`;
- `TrainingRunReadinessReport`;
- `TrainingContractCoverageReport`.

Application:

- `TrainingRunPlanReader` port;
- `TrainingTrajectoryReader` port;
- `AssessKmpMcpTrainingContractCoverageUseCase`;
- `PrepareTrainingRunUseCase`.

Infra:

- adapters for training manifests, filesystem artifacts, Kubernetes jobs, or
  future training providers.

### Replay

Domain:

- `ReplayTrajectory`;
- `ReplayPrediction`;
- `ReplayActionDecision`;
- `ReplayRow`;
- `ReplayRunReport`;
- replay counters, page metadata, partial-result flags, missing/extra ref
  detection, and latency summary metrics.

Application:

- `ReplayToolCaller` port;
- `ReplayMcpPredictionsUseCase`;
- progress observer hook for bounded long replays.

Infra:

- `JsonlReplayReader`, which reads trajectories and predictions from JSONL;
- `ReplayOutputWriter`, which writes `summary.json` and `results.jsonl`.

CLI:

- `underpass-operator-replay-cli`, binary
  `underpass_operator_mcp_replay`;
- current composition root that connects the replay use case to the existing
  MCP adapter and gRPC endpoint.

## Rules

1. No new Python validators.
2. No silent fallback.
3. Unsupported mode, tool, source, or action shape fails fast.
4. DTOs are only boundary shapes.
5. Domain logic must use typed value objects before making decisions.
6. JSON is only an adapter format.
7. JSON parsing and emission live in infra mappers/adapters.
8. Metrics and reports live in domain.
9. Application orchestrates use cases; it does not own metrics semantics.
10. Infra does not own domain rules.
11. Bounded contexts must not depend on each other.
12. Operator domain/application/infra crates must not depend on kernel crates.
13. CLI crates may depend on the current adapter only as composition roots, and
    must not own domain decisions.
14. `rehydration-testkit` must not contain Operator binaries, Operator facades,
    or dependencies on `underpass-operator-*`.
15. Unit coverage comes before large benchmark runs.

## Current Slice

Implemented in this cut:

- shared hexagonal context;
- synthetic creation hexagonal context;
- evaluation hexagonal context;
- training hexagonal context;
- replay hexagonal context plus CLI composition root;
- canonical KMP/MCP capability inventory in shared domain;
- one use case per active bounded context for KMP/MCP contract capability
  coverage;
- explicit trajectory-builder use case for canonical Operator trajectories;
- metrics moved into domain;
- policy evaluator scoring moved into `evaluation-domain`;
- policy evaluator orchestration moved into `evaluation-application`;
- policy and coverage JSONL reading moved into `evaluation-infra`;
- live MCP replay decisions, rows, counters, page metadata, and summary metrics
  moved into `replay-domain`;
- live MCP replay orchestration moved into `replay-application`;
- live MCP replay JSONL readers and report writers moved into `replay-infra`;
- live MCP replay executable moved from `rehydration-testkit` to
  `underpass-operator-replay-cli`;
- synthetic conformance trajectory builder moved to
  `underpass-operator-synthetic-cli`;
- policy evaluation, contract coverage, and LLM baseline CLIs moved to
  `underpass-operator-evaluation-cli`;
- MemoryArena and LongMemEval trajectory adapters moved to
  `underpass-operator-benchmark-adapters-cli`;
- Operator facades, binaries, and dependencies removed from
  `rehydration-testkit`;
- workspace dependencies for all contexts;
- unit tests for context boundaries and basic domain/application behavior.

The current crate set is:

```text
underpass-operator-shared-domain
underpass-operator-shared-contract
underpass-operator-shared-application
underpass-operator-shared-infra

underpass-operator-synthetic-domain
underpass-operator-synthetic-application
underpass-operator-synthetic-infra
underpass-operator-synthetic-cli

underpass-operator-evaluation-domain
underpass-operator-evaluation-application
underpass-operator-evaluation-infra
underpass-operator-evaluation-cli

underpass-operator-training-domain
underpass-operator-training-application
underpass-operator-training-infra

underpass-operator-replay-domain
underpass-operator-replay-application
underpass-operator-replay-infra
underpass-operator-replay-cli

underpass-operator-benchmark-adapters-cli
```

## Parity Status

This refactor now moves the KMP/MCP Operator action validator into
`underpass-operator-shared-domain`. `rehydration-testkit` no longer keeps
Operator helper facades or Operator binaries. Current Operator execution paths
use the dedicated Operator CLI crates directly.

| Area | Status |
| --- | --- |
| Tool names by mode | Covered in `shared-domain`. |
| Stable read/write/full tool ordering | Covered by domain unit tests. |
| DTO shapes for raw trajectories and actions | Covered in `shared-contract`. |
| DTO/domain mapping | Covered in `shared-infra`. |
| Dataset preflight metrics | Domain-owned in `shared-domain`; calculated by shared application use case. |
| Synthetic generation metrics | Domain-owned in `synthetic-domain`. |
| Canonical Operator trajectory builder | Covered by `BuildOperatorTrajectoryUseCase`; rejects trajectories that do not match the KMP/MCP case spec. |
| Synthetic KMP/MCP capability planning | Covered by `PlanKmpMcpSyntheticCasesUseCase`. |
| Evaluation metrics | Domain-owned in `evaluation-domain`. |
| Evaluation KMP/MCP capability coverage | Covered by `EvaluateKmpMcpContractCoverageUseCase`. |
| Training readiness metrics | Domain-owned in `training-domain`. |
| Training KMP/MCP capability coverage | Covered by `AssessKmpMcpTrainingContractCoverageUseCase`. |
| Full KMP/MCP action schema validation | Covered in `shared-domain`; consumed directly by Operator CLIs. |
| Boundedness validation per tool | Covered in `shared-domain`; consumed directly by Operator CLIs. |
| Dimension scope semantics | Covered in `shared-domain`; current/abouts/all_abouts fail-fast rules remain centralized. |
| Raw access policy | Covered in `shared-domain`; safe profile rejects raw inspect/raw refs. |
| Relation quality/read-context proof validation | Covered in `shared-domain`; relation vocabulary is Operator-owned and kernel-independent. |
| Coverage profile/capability model | Covered in `evaluation-domain`. |
| Coverage reporter use case | Covered in `evaluation-application`; current CLI delegates. |
| Coverage JSONL row reading/observer | Covered in `evaluation-infra`; current CLI delegates. |
| Policy evaluator scoring/use case | Covered in `evaluation-domain` and `evaluation-application`; current CLI delegates. |
| Policy trajectory/prediction JSONL readers | Covered in `evaluation-infra`; current CLI delegates. |
| MCP replay decision/metrics | Covered in `replay-domain`. |
| MCP replay use case/port | Covered in `replay-application` through `ReplayToolCaller`. |
| MCP replay JSONL/report adapters | Covered in `replay-infra`. |
| MCP replay executable | Covered in `underpass-operator-replay-cli`; old `rehydration-testkit` binary removed. |

Correct claim for this cut:

```text
Operator now has shared, synthetic, evaluation, training, and replay bounded
contexts. The KMP/MCP action contract lives in shared-domain. Contract coverage
profile semantics and policy evaluation scoring live in the evaluation bounded
context. Coverage row observation and policy JSONL reading live in evaluation
infra adapters. Synthetic conformance trajectories are built by the synthetic
bounded context and exposed through its CLI. Benchmark artifact adapters are
outside the Operator core. Live MCP replay lives in the Operator replay bounded
context, with a dedicated CLI composition root for the current kernel MCP
adapter. `rehydration-testkit` no longer owns Operator code.
```

Incorrect claim:

```text
Benchmark exporters are part of the Operator core architecture.
```

## P0 Migration Plan

1. Keep benchmark exporters outside the Operator bounded contexts. They live in
   `underpass-operator-benchmark-adapters-cli` as experiment adapters.
2. Build canonical Operator trajectories from the KMP/MCP contract and use-case
   specs, not from benchmark artifact shapes.
3. Keep replay CLI dependency on `rehydration-mcp` isolated until a pure remote
   MCP client adapter exists.

## Non Goals

This refactor is not a change to KMP semantics.

It is not a new benchmark strategy.

It is not a model-quality improvement by itself.

It is the architecture needed so model-quality work is not polluted by invalid
training rows, permissive evaluators, mixed responsibilities, or hidden adapter
behavior.
