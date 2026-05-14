# Operator MCP/API Contract Gap Audit - 2026-05-14

Esta auditoria documenta todos los gaps detectados entre:

- la API canonica KMP (`KernelMemoryService` gRPC);
- la entrada agentica MCP que usa un LLM;
- el contrato estricto que hoy usa el Operator;
- la cobertura real de las trayectorias MemoryArena V6 holdout20.

Objetivo de producto:

```text
El Operator debe aprender a usar KMP al maximo.
No debe aprender solo un subconjunto que pasa un benchmark.
```

Para el Operator, MCP es la forma agentica de consumir KMP. Por tanto, cuando se
diga "contrato MCP/API" en este documento, significa:

```text
MCP tools schema
  + runtime mapping MCP -> KernelMemoryService gRPC
  + semantica real del kernel desplegado
```

## Fuentes De Verdad

| Capa | Archivo |
| --- | --- |
| API gRPC canonica | `api/proto/underpass/rehydration/kernel/v1beta1/memory.proto` |
| MCP tools schema | `crates/rehydration-mcp/src/protocol.rs` |
| MCP -> gRPC request mapping | `crates/rehydration-mcp/src/grpc/requests/*.rs` |
| MCP structured output mapping | `crates/rehydration-mcp/src/kmp.rs` |
| Operator Rust contract validator | `crates/rehydration-testkit/src/kernel_operator.rs` |
| Operator Python predictor validator | `scripts/operator/predict_operator_sft.py` |
| Operator trajectories exporter | `crates/rehydration-testkit/src/bin/kernel_operator_trajectory_export.rs` |
| Operator policy evaluator | `crates/rehydration-testkit/src/bin/kernel_operator_policy_eval.rs` |
| Operator live replay | `crates/rehydration-testkit/src/bin/kernel_operator_mcp_replay.rs` |

## Situacion Actual

El resultado MemoryArena V6 holdout20 es fuerte, pero estrecho.

Offline strict policy eval:

| Metric | Value |
| --- | ---: |
| Held-out decisions | 1,124 |
| Exact action accuracy | 1.0000 |
| Tool accuracy | 1.0000 |
| Primary ref accuracy | 1.0000 |
| Scope accuracy | 1.0000 |
| Stop accuracy | 1.0000 |
| Missing predictions | 0 |
| Invalid predictions | 0 |
| Unbounded tool calls | 0 |

Live MCP replay:

| Metric | Value |
| --- | ---: |
| Executed MCP tool calls | 976 |
| Stop actions | 148 |
| Successful tool calls | 976 |
| Failed tool calls | 0 |
| Missing expected refs | 0 |
| Partial result rows | 424 |

Esto demuestra que el Operator opera correctamente el subconjunto cubierto por
el holdout.

No demuestra todavia que opere KMP completo.

## P0: Cobertura 100% Del Contrato MCP/API

Este punto pasa a ser P0.

Si el Operator se coloca delante de KMP/MCP, no puede estrechar la API real sin
que eso sea una decision explicita de perfil. Si el modelo solo sabe usar una
parte de MCP, entonces el modelo no esta acelerando KMP completo: esta creando
una puerta mas pequena que puede mutilar capacidades reales del sistema.

Por tanto necesitamos una metrica de cobertura del contrato:

```text
operator_contract_coverage = capacidades MCP/API soportadas por el Operator
                             / capacidades MCP/API requeridas por el perfil
```

El objetivo para cualquier perfil publicable es:

```text
100% contract coverage
100% dataset target coverage
100% live replay coverage
```

Esto no significa que un unico modelo tenga que escribir, leer y auditar desde
el primer dia. Significa que cada perfil declarado debe estar completo.

Perfiles razonables:

| Perfil | Alcance |
| --- | --- |
| `operator-read` | `wake`, `ask`, `near`, `goto`, `rewind`, `forward`, `trace`, `inspect`, `stop` |
| `operator-audit` | `operator-read` + raw audit refs/raw inspect bajo politica explicita |
| `operator-write` | `write_memory`/`ingest` + lectura previa para prueba de relacion |
| `operator-full` | Todo el surface MCP/KMP expuesto al LLM |

La cobertura debe ser visible en CI/local checks. Si una herramienta nueva entra
en MCP, o si cambia la semantica de dimensiones, cursores, paginacion o raw
access, la cobertura debe bajar hasta que exista soporte del Operator y dataset
que lo ejercite.

Medicion actual del reporter `kernel_operator_contract_coverage`:

| Scope | Coverage | Lectura |
| --- | ---: | --- |
| MCP global tools | 80.00% | `kernel_ingest` y `kernel_write_memory` no pertenecen al perfil lector actual. |
| `operator-read` contract | 100.00% | El contrato ya puede expresar el perfil lector completo. |
| `operator-full` contract | 85.71% | Falta escribir/validar `kernel_ingest`, `kernel_write_memory`, relation quality y read_context proof. |
| MemoryArena V6 target capabilities | 41.67% | El dataset actual no cubre todo el perfil lector. |

El dato importante es el tercero. Aunque el contrato ya pueda expresar
`operator-read`, el modelo no ha sido entrenado/evaluado con suficientes casos
para decir que domina ese perfil.

## P0: Datasets Por Caso De Uso MCP/API

El gap principal ahora no es solo de modelo. Es de datos.

Necesitamos sets de entrenamiento/evaluacion por caso de uso de la API/MCP para
cubrir el 100% del contrato declarado. MemoryArena V6 valida un subconjunto muy
util, pero no contiene todos los casos que KMP expone.

Suites necesarias:

| Suite | Objetivo |
| --- | --- |
| `kmp-read-wake` | Aprender cuando usar `kernel_wake` para continuar trabajo desde memoria. |
| `kmp-read-ask` | Aprender `kernel_ask` con `evidence_or_unknown`, `show_conflicts` y `best_effort`. |
| `kmp-temporal-ref` | `near/goto/rewind/forward` con cursor por `ref`. |
| `kmp-temporal-time` | `near/goto/rewind/forward` con cursor por `time`. |
| `kmp-temporal-sequence` | `near/goto/rewind/forward` con cursor por `sequence`. |
| `kmp-dimensions-mode` | `all`, `only`, `except`, incluyendo casos invalidos fail-fast. |
| `kmp-dimensions-scope` | `current_about`, `abouts`, `all_abouts`, incluyendo `ABOUTS` sin lista como error. |
| `kmp-trace-pagination` | Primera pagina y continuacion con `page.cursor`. |
| `kmp-window-policy` | Expandir ventana, reducir ventana, cambiar de herramienta y parar. |
| `kmp-inspect-policy` | Inspect ligero, inspect tipado completo, raw=false por defecto. |
| `kmp-audit-raw` | Raw refs/raw inspect solo si se declara perfil auditor. |
| `kmp-write-memory` | Escribir texto + relacion + why + evidencia + read_context proof. |
| `kmp-ingest-canonical` | Ingest canonico, idempotency, dimensiones, entradas, relaciones y evidencia. |

Cada suite debe producir:

- trajectories model-facing sin gold leakage;
- targets estrictos;
- policy eval offline;
- replay real MCP/gRPC;
- cobertura por herramienta;
- cobertura por cursor;
- cobertura por modo/scope de dimensiones;
- cobertura de paginacion;
- cobertura de politica de ventana;
- errores esperados fail-fast cuando el caso sea invalido.

Hasta tener estas suites, no se debe afirmar que Operator sabe usar KMP al
maximo. La afirmacion correcta sigue siendo:

```text
Operator sabe operar el subconjunto KMP/MCP cubierto por el dataset actual.
```

## Herramientas MCP/API

MCP expone actualmente:

| MCP tool | gRPC/API target | Clase |
| --- | --- | --- |
| `kernel_ingest` | `KernelMemoryService.Ingest` | write canonical |
| `kernel_write_memory` | compila a `kernel_ingest` | write helper |
| `kernel_wake` | `KernelMemoryService.Wake` | read/continuation |
| `kernel_ask` | `KernelMemoryService.Ask` | deterministic evidence QA |
| `kernel_goto` | `KernelMemoryService.Goto` | temporal movement |
| `kernel_near` | `KernelMemoryService.Near` | temporal neighborhood |
| `kernel_rewind` | `KernelMemoryService.Rewind` | temporal movement |
| `kernel_forward` | `KernelMemoryService.Forward` | temporal movement |
| `kernel_trace` | `KernelMemoryService.Trace` | proof/path traversal |
| `kernel_inspect` | `KernelMemoryService.Inspect` | node/detail inspection |

El Operator actual permite solo:

| Operator action | Estado |
| --- | --- |
| `kernel_ask` | Permitido por contrato, no cubierto en V6 holdout. |
| `kernel_goto` | Permitido por contrato, no cubierto en V6 holdout. |
| `kernel_near` | Cubierto y replay real limpio. |
| `kernel_rewind` | Permitido por contrato, no cubierto en V6 holdout. |
| `kernel_forward` | Permitido por contrato, no cubierto en V6 holdout. |
| `kernel_trace` | Cubierto, sin page continuation en targets. |
| `kernel_inspect` | Cubierto con `raw=false`. |
| `stop` | Cubierto. |
| `kernel_wake` | No permitido. |
| `kernel_ingest` | No permitido. |
| `kernel_write_memory` | No permitido. |

Decision pendiente:

```text
Si queremos un Operator que use KMP al maximo, hay que decidir si hay un solo
Operator full-KMP o perfiles separados: read-operator, writer-operator,
audit-operator.
```

## Cobertura Real Del Holdout V6

Target actions en MemoryArena V6 holdout20:

| Target | Count |
| --- | ---: |
| `kernel_near` | 424 |
| `kernel_inspect` | 424 |
| `kernel_trace` | 128 |
| `stop` | 148 |

Por modo:

| Mode | Target | Count |
| --- | --- | ---: |
| `read` | `kernel_near` | 148 |
| `read` | `kernel_inspect` | 148 |
| `read` | `kernel_trace` | 128 |
| `read` | `stop` | 148 |
| `write_context_read` | `kernel_near` | 276 |
| `write_context_read` | `kernel_inspect` | 276 |

Allowed tools en cada trajectory incluian:

- `kernel_ask`;
- `kernel_forward`;
- `kernel_goto`;
- `kernel_inspect`;
- `kernel_near`;
- `kernel_rewind`;
- `kernel_trace`.

Pero el target no uso:

- `kernel_ask`;
- `kernel_goto`;
- `kernel_rewind`;
- `kernel_forward`.

Esto significa que el modelo puede tener esas tools en `allowed_tools`, pero el
score actual no prueba que sepa usarlas.

## Gaps De Tool Coverage

### `kernel_wake`

API/MCP:

- existe;
- requiere `about`;
- acepta `role`, `intent`, `dimensions`, `depth`, `budget`;
- devuelve un wake packet para continuar trabajo desde memoria.

Operator:

- no lo permite;
- no hay targets;
- no hay policy eval;
- no hay replay.

Gap:

```text
El Operator no sabe decidir cuando despertar contexto en vez de hacer ask/near.
```

P0/P1 decision:

- si `wake` es parte del operador lector, meterlo en el perfil read;
- si `wake` es un helper de cliente, dejarlo fuera pero documentarlo.

### `kernel_ask`

API/MCP:

- requiere `about`, `question`;
- `answer_policy` es opcional y default `evidence_or_unknown`;
- `dimensions` es opcional;
- `budget` es opcional;
- devuelve evidencia determinista, no respuesta generativa libre.

Operator:

- lo permite;
- exige `answer_policy`;
- exige `dimensions`;
- no aparece en MemoryArena V6 target;
- LongMemEval v10 lo ejercito y expuso el fallo `final_refs` dentro de
  `kernel_ask.arguments`, ya corregido por predictor estricto.

Gaps:

- falta cobertura estable en un holdout publicable;
- falta medir cuando conviene `ask` frente a `near + inspect`;
- falta policy para `show_conflicts` y `best_effort`;
- falta QA multi-session limpio sin depender del reader generativo.

### `kernel_near`

API/MCP:

- requiere `about`, `around`;
- `around` puede ser por `ref`, `time` o `sequence`;
- acepta `window`, `limit`, `dimensions`, `include`, `depth`, `budget`;
- devuelve `entries`, `proof`, `coverage`, `raw_refs`, `warnings`, `page`.

Operator:

- cubierto;
- exige cursor `around.ref`;
- exige `window`, `limit`, `dimensions`, `include`, `budget`;
- aprende dos presets:
  - `read`: entries 12, tokens 2400, depth 3, before 6, after 0;
  - `write_context_read`: entries 8, tokens 1800, depth 2, before 3, after 0.

Gaps:

- no usa cursor `time`;
- no usa cursor `sequence`;
- no aprende expansion/reduccion dinamica de ventana;
- no aprende continuation policy sobre `page.has_more`;
- no aprende `after_entries > 0` en este holdout;
- no aprende scopes `abouts` o `all_abouts`.

### `kernel_goto`

API/MCP:

- salta al estado en un cursor temporal;
- cursor puede ser `ref`, `time`, `sequence`;
- acepta `window`, `limit`, `dimensions`, `include`, `budget`.

Operator:

- permitido por contrato;
- no cubierto en V6 targets;
- validador solo acepta `at.ref`.

Gaps:

- no hay training/eval real;
- no hay cursor time/sequence;
- no hay decision policy para "estado conocido en ese momento";
- no hay benchmark de known-at usando `goto`.

### `kernel_rewind`

API/MCP:

- mueve hacia atras desde un cursor;
- cursor puede ser `ref`, `time`, `sequence`;
- acepta `window`, `limit`, `dimensions`, `include`, `budget`.

Operator:

- permitido por contrato;
- no cubierto en V6 targets;
- validador solo acepta `from.ref`.

Gaps:

- no hay training/eval real;
- no hay policy para buscar decisiones previas;
- no hay policy de profundidad temporal;
- no hay cursor time/sequence.

### `kernel_forward`

API/MCP:

- mueve hacia adelante desde un cursor;
- cursor puede ser `ref`, `time`, `sequence`;
- acepta `window`, `limit`, `dimensions`, `include`, `budget`.

Operator:

- permitido por contrato;
- no cubierto en V6 targets;
- validador solo acepta `from.ref`.

Gaps:

- no hay training/eval real;
- no hay policy para buscar cambios posteriores;
- no hay policy para actualizar estado vigente;
- no hay cursor time/sequence.

### `kernel_trace`

API/MCP:

- requiere `from`, `to`;
- `goal`, `role`, `budget`, `page` son opcionales;
- devuelve `trace`, `warnings`, `page`;
- `page.has_more=true` implica continuar con `page.next_cursor`.

Operator:

- cubierto;
- exige `budget`;
- permite `page`, pero no hay targets con `page`;
- V6 target/predictions: 128 `kernel_trace`, todos sin `page`.

Gaps:

- no aprende continuation de trace;
- no aprende `page.cursor`;
- no aprende cuando aumentar `page.entries`;
- no aprende cuando parar con trace parcial;
- no consume warnings/page como senal de siguiente decision.

### `kernel_inspect`

API/MCP:

- requiere `ref`;
- `include` es opcional;
- `include.raw=true` esta soportado por MCP/API;
- devuelve object, links, evidence, warnings, raw.

Operator:

- cubierto;
- exige `include`;
- exige `raw=false`;
- target actual siempre:

```json
{"details": true, "incoming": true, "outgoing": true, "raw": false}
```

Gaps:

- no sabe pedir raw audit refs;
- no aprende variantes ligeras (`details` only, links only);
- no aprende tradeoff coste/detalle;
- no aprende inspect de raw cuando auditoria lo necesita.

Decision:

- `raw=false` es conservador y razonable para seguridad;
- si queremos Operator auditor, `raw=true` debe ser otro modo/perfil con tests.

### `kernel_ingest`

API/MCP:

- low-level canonical write;
- requiere `about`, `memory`, `idempotency_key`;
- escribe dimensiones, entries, relations, evidence, provenance.

Operator:

- no permitido;
- no entrenado.

Gap:

```text
El Operator actual no sabe escribir memoria canonical KMP.
```

Decision:

- probablemente no debe entrar en el primer read-operator;
- si se mete, debe ser un writer-operator separado con validacion de relaciones.

### `kernel_write_memory`

API/MCP:

- helper writer-friendly;
- valida intent, relation quality, read_context proof;
- compila a `kernel_ingest`;
- soporta `dry_run` y commit.

Operator:

- no permitido;
- no entrenado;
- el smart writer actual lo usa como pipeline externo, no como decision del
  Operator pequeno.

Gap:

```text
El Operator no sabe todavia escribir memoria inteligente por MCP.
```

Decision:

- separar `Operator-read` de `Operator-write`;
- no mezclar hasta que `Operator-read` domine KMP temporal/paginado.

## Gaps De Campo Y Semantica

### Campos Opcionales En API, Requeridos Por Operator

| Tool | API/MCP | Operator |
| --- | --- | --- |
| `kernel_ask.answer_policy` | opcional, default `evidence_or_unknown` | requerido |
| `kernel_ask.dimensions` | opcional | requerido |
| temporal `dimensions` | opcional | requerido |
| temporal `include` | opcional | requerido |
| temporal `limit` | opcional | requerido |
| temporal `budget` | opcional/default | requerido |
| temporal `window` | opcional | requerido |
| `kernel_trace.budget` | opcional/default | requerido |
| `kernel_trace.page` | opcional | opcional |
| `kernel_inspect.include` | opcional | requerido |

Esto es un subconjunto estricto. Es aceptable para bounded tool-use, pero debe
quedar documentado como un perfil:

```text
Operator strict profile != todo el espacio valido de MCP/API.
```

### Cursor Temporal

| Cursor | API/MCP | Operator |
| --- | --- | --- |
| `ref` | soportado | soportado |
| `time` | soportado | no soportado |
| `sequence` | soportado | no soportado |

Este es gap claro para usar KMP al maximo.

### Dimension Selection

MCP/gRPC valida semantica:

| Regla | API/MCP runtime | Operator validator actual |
| --- | --- | --- |
| `mode=all` no lleva `include/exclude` | valida | no valida completo |
| `mode=only` requiere `include` | valida | no valida completo |
| `mode=except` requiere `exclude` | valida | no valida completo |
| `scope=current_about` no lleva `abouts` | valida | no valida completo |
| `scope=abouts` requiere `abouts` no vacio | valida | no valida completo |
| `scope=all_abouts` no lleva `abouts` | valida | no valida completo |

Este es gap P0 de contrato, porque puede inflar metricas offline:

```text
Una accion podria pasar policy eval y fallar luego en MCP/gRPC.
```

Accion:

- mover/duplicar las reglas semanticas de `dimension_selection_from_arguments`
  al validador del Operator;
- anadir fixtures validos e invalidos;
- contar `invalid_prediction_reasons` por regla.

### Temporal Window

API/MCP:

- `window.before_entries` opcional;
- `window.after_entries` opcional;
- si faltan, default 0;
- `before_seconds/after_seconds` se rechazan en este corte.

Operator:

- exige `before_entries`;
- exige `after_entries`;
- no usa `after_entries > 0` en V6;
- no soporta seconds, igual que API runtime.

Gap:

- no domina windows simetricas;
- no domina forward-looking window;
- no aprende reducir/ampliar window.

### Limit

API/MCP:

- `limit.entries` opcional;
- `limit.tokens` opcional;
- si faltan, el kernel/mapping puede aplicar default.

Operator:

- para temporal exige ambos;
- valida boundedness maxima:
  - entries <= 64;
  - tokens <= 16,000.

Gap:

- no aprende tradeoff entries/tokens;
- no aprende token budget dinamico;
- no aprende reducir tokens por coste;
- no aprende aumentar entries si `has_more=true`.

### Budget

API/MCP:

- opcional;
- mapping aplica defaults:
  - ask: 2400 tokens, depth 2;
  - temporal: 2400 tokens, depth 3;
  - trace: 1600 tokens, depth 1.

Operator:

- exige budget en temporal y trace;
- acepta `tokens`, `depth`, `detail`;
- no entrena `detail`.

Gap:

- no aprende `budget.detail`;
- no aprende presupuesto segun complejidad;
- no aprende stop por presupuesto bajo.

### Include

API/MCP:

- temporal include opcional;
- inspect include opcional;
- defaults false/true segun mapping.

Operator:

- exige include explicito;
- temporal actual usa evidence=true, relations=true, raw_refs=false;
- inspect actual usa details=true, incoming=true, outgoing=true, raw=false.

Gaps:

- no aprende variants ligeras;
- no aprende `raw_refs=true`;
- no aprende `raw=true`;
- no aprende coste/beneficio de links/evidence.

## Gaps De Output MCP/API Frente A Visible State Del Operator

MCP/API devuelve mas informacion de la que el Operator ve hoy en V6.

### Temporal Output

API/MCP puede devolver:

- `summary`;
- `temporal.requested`;
- `temporal.resolved`;
- `coverage.requested`;
- `coverage.included`;
- `coverage.missing`;
- `entries`;
- `proof`;
- `warnings`;
- `raw_refs`;
- `page`.

Visible state V6 contiene principalmente:

- `candidate_refs`;
- `candidate_ref_details`;
- `current_ref`;
- `known_refs`;
- `last_observed_refs`;
- `last_tool`;
- `remaining_budget`;
- `writer` metadata.

Gaps:

- no incluye `coverage.missing`;
- no incluye warnings;
- no incluye page metadata en V6;
- no incluye token/page cost real;
- no incluye proof completeness;
- no incluye enough/sufficient evidence labels;
- no incluye razon explicita de por que target amplia/reduce/para.

### Trace Output

API/MCP devuelve:

- `trace`;
- `warnings`;
- `page`.

Gaps:

- Operator no aprende continuacion de trace;
- no aprende `page.next_cursor`;
- no aprende si un trace parcial ya basta;
- no aprende a pedir mas path cuando falta causalidad.

### Inspect Output

API/MCP devuelve:

- object;
- links incoming/outgoing;
- evidence;
- warnings;
- raw.

Gaps:

- visible state comprime esto a refs/candidates;
- no aprende variaciones de include;
- no aprende auditoria raw;
- no aprende usar warning como decision.

## Gaps De Evaluacion

El evaluator actual mide:

- action type;
- tool;
- primary refs;
- scope;
- stop;
- exact action;
- invalid predictions;
- unbounded tool calls.

No mide todavia:

- `dimension_semantic_validity`;
- `cursor_mode_accuracy`;
- `time_cursor_accuracy`;
- `sequence_cursor_accuracy`;
- `window_policy_accuracy`;
- `expand_window_accuracy`;
- `shrink_window_accuracy`;
- `continue_page_accuracy`;
- `stop_when_sufficient_accuracy`;
- `over_read_rate`;
- `under_read_rate`;
- `raw_access_policy_accuracy`;
- `wake_vs_ask_vs_near_accuracy`;
- `write_dry_run_vs_commit_accuracy`;
- `reader/plugin handoff correctness`.

P0 evaluator additions:

| Metric | Meaning |
| --- | --- |
| `api_contract_valid` | action passes full MCP/gRPC semantic rules |
| `operator_profile_valid` | action passes stricter operator profile |
| `cursor_mode_accuracy` | ref/time/sequence choice matches target |
| `window_shape_accuracy` | before/after entries match target class |
| `limit_policy_accuracy` | entries/tokens match target class |
| `continue_page_accuracy` | follows `page.next_cursor` when target says continue |
| `stop_when_sufficient_accuracy` | stops after enough evidence |
| `over_read_rate` | reads more context than target policy |
| `under_read_rate` | stops or narrows before evidence is sufficient |
| `raw_access_violation_rate` | asks raw when policy forbids it |

## Gaps De Dataset

Current V6 dataset:

- no target `kernel_ask`;
- no target `kernel_goto`;
- no target `kernel_rewind`;
- no target `kernel_forward`;
- no target `kernel_wake`;
- no target write tools;
- no time cursor;
- no sequence cursor;
- no trace page continuation;
- no visible `last_result_page`;
- no visible `last_result_partial`;
- no dynamic window target labels;
- no all_abouts/abouts scope policy;
- no raw access policy.

This dataset is excellent for the first read/navigation proof, but not enough
for full KMP operation.

## P0: Full KMP Operation Slice

The next P0 is not "run a bigger benchmark". It is:

```text
teach and verify the Operator can operate the KMP API surface, not only the
MemoryArena V6 subset.
```

### P0.1 Align Contract Validators

Implement one shared operator contract profile:

- exact action shape;
- exact tool schemas;
- full DimensionSelection semantics;
- boundedness;
- raw access policy;
- cursor mode rules;
- page continuation rules.

Acceptance:

- Rust validator catches every MCP/gRPC runtime semantic failure we can know
  statically;
- Python predictor validator matches Rust;
- fixtures cover valid/invalid cases for every tool;
- policy eval reports invalid reasons by category.

### P0.2 Add API-Conformance Trajectories

Create a small "KMP operator conformance" dataset before scaling benchmarks.

Required cases:

| Case | Required target |
| --- | --- |
| Ask from current about | `kernel_ask` |
| Ask with conflicts | `kernel_ask(answer_policy=show_conflicts)` |
| Wake continuation | `kernel_wake` if included in read profile |
| Near by ref | `kernel_near(around.ref)` |
| Near by time | `kernel_near(around.time)` |
| Near by sequence | `kernel_near(around.sequence)` |
| Goto known-at | `kernel_goto(at.time/ref/sequence)` |
| Rewind previous decision | `kernel_rewind(from.ref/time/sequence)` |
| Forward later update | `kernel_forward(from.ref/time/sequence)` |
| Trace first page | `kernel_trace(page.entries=N)` |
| Trace continuation | `kernel_trace(page.cursor=next_cursor)` |
| Inspect light | `kernel_inspect(details=true, links=false)` |
| Inspect full typed | `kernel_inspect(details=true,incoming=true,outgoing=true,raw=false)` |
| Inspect raw audit | optional profile, `raw=true` |
| Stop sufficient evidence | `stop` |
| Stop unknown | `stop(answer_policy=evidence_or_unknown, final_refs=[])` |

### P0.3 Add Dynamic Window/Page Trajectories

Required policy labels:

- `expand_window`;
- `shrink_window`;
- `continue_page`;
- `switch_to_inspect`;
- `switch_to_trace`;
- `stop_sufficient`;
- `stop_budget`;
- `ask_instead_of_near`;
- `goto_known_at`;
- `forward_find_update`;
- `rewind_find_cause`.

Visible state must include:

- last tool;
- last observed refs;
- last observed count;
- last page:
  - returned;
  - total;
  - has_more;
  - next_cursor;
- last coverage:
  - included;
  - missing;
- last warnings;
- remaining tool calls;
- remaining context budget;
- evidence sufficiency label when generated by test harness;
- target rationale for audit, not model prompt, unless explicitly allowed.

### P0.4 Decide Profiles

Recommended profiles:

| Profile | Tools |
| --- | --- |
| `operator-read` | wake, ask, near, goto, rewind, forward, trace, inspect, stop |
| `operator-audit` | read profile + raw refs/raw inspect under explicit policy |
| `operator-write` | write_memory, possibly ingest, plus read_context proof tools |

Do not mix write into first release unless read profile is stable.

### P0.5 Replay Against Real MCP/API

Every accepted profile needs:

- offline strict policy eval;
- de-anonymized raw predictions;
- live MCP replay;
- zero MCP failures;
- zero missing expected refs;
- zero invalid predictions;
- zero unbounded calls;
- page continuation verified;
- temporal cursor modes verified.

## Acceptance Criteria For "Operator Knows KMP"

The phrase "Operator knows KMP" is allowed only when:

1. It covers every tool in the selected profile.
2. It covers every cursor mode in that profile.
3. It covers dimension scopes:
   - current_about;
   - abouts with non-empty list;
   - all_abouts explicit.
4. It covers dimension modes:
   - all;
   - only;
   - except.
5. It demonstrates dynamic window/page behavior.
6. It handles raw access policy explicitly.
7. It passes strict validator and live replay.
8. It reports all metrics by tool and by policy case.
9. It does not rely on benchmark gold fields.
10. It does not silently fallback.

Until then, the correct claim is narrower:

```text
Operator currently operates a strict bounded read/navigation subset of KMP,
validated on MemoryArena V6 holdout20 and live MCP replay.
```

## Immediate Work Items

P0 implementation checklist:

1. Extend `kernel_operator_action_contract_error` with full dimension semantic
   validation.
2. Add tests for invalid dimensions:
   - `mode=only` without include;
   - `mode=except` without exclude;
   - `scope=abouts` without abouts;
   - `scope=all_abouts` with abouts.
3. Add cursor mode support to Operator validator:
   - ref;
   - time;
   - sequence.
4. Decide if operator profile still requires ref-only for first release. If
   yes, document that as profile restriction, not API limitation.
5. Add page-aware visible state to trajectory export from live/replay rows.
6. Generate KMP conformance trajectories.
7. Add evaluator metrics for dynamic window/page policy.
8. Run a small conformance SFT/eval before any larger MemoryArena run.
9. Replay conformance predictions through public TLS MCP endpoint.
10. Only after this, scale MemoryArena.
