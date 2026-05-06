# Reusable Interpretation Plugins

Status: first reusable testkit slice.

The kernel retrieves deterministic evidence. Interpretation plugins consume that
evidence and produce typed, auditable derivations. This keeps benchmark,
business, and domain operators outside kernel core while still making them easy
to reuse from readers, agents, and evaluation harnesses.

## Boundary

Kernel responsibilities:

- ingest memories;
- preserve `about`, dimension, temporal, relation, trace, and inspect state;
- retrieve evidence with stable refs.

Plugin responsibilities:

- interpret evidence values;
- decide which extracted values are operands only when an upstream reader or
  agent marks them as `include`, `exclude`, or `context`;
- normalize money, dates, numbers, units, and entities;
- run deterministic operations over typed operands;
- return answer, value, refs, excluded refs, and diagnostics.

Plugins must fail fast on incompatible operands. For example, a money sum cannot
silently mix `USD` and `EUR`; a date sum is invalid; a date difference returns a
typed number in days.

## Implemented Slice

The first implementation lives in
`crates/rehydration-testkit/src/interpretation_plugins.rs` and is exported from
`rehydration-testkit`.

Implemented plugins:

- `MoneyValuePlugin`: extracts money mentions such as `$1,200.50`, `EUR 30`,
  `30 dollars`, and normalizes them into `InterpretedValue::Money`.
- `DateValuePlugin`: extracts ISO-like dates and simple named dates such as
  `2026-05-06`, `05/06/2026`, and `May 6, 2026`.
- `ValueOperationPlugin`: computes `sum`, `count`, `average`, `difference`,
  `max_by`, `list`, and explicit `unknown` over `DerivationOperand` values.

The operation input is deliberately separate from extraction. A money mention is
not automatically a sum operand. The reader or agent must create a derivation
request that labels each candidate as `include`, `exclude`, or `context`.

## Minimal Flow

1. Ask the kernel for context and keep the refs.
2. Run value extractors on the retrieved evidence.
3. Let a reader or agent select operands against the question.
4. Run `ValueOperationPlugin`.
5. Optionally write the derived result back to memory with provenance
   `derived_from: [refs...]`.

Example shape:

```rust
use rehydration_testkit::{
    DateValuePlugin, DerivationOperand, DerivationOperation, DerivationRequest,
    EvidenceFragment, EvidenceInterpretationInput, EvidenceValuePlugin,
    InterpretedValue, MoneyValuePlugin, OperandRole, ValueOperationPlugin,
};

let evidence = EvidenceInterpretationInput::new(vec![EvidenceFragment::new(
    "turn:42",
    "Paid $120 on 2026-05-01 and $65 on 2026-05-06.",
)]);

let money = MoneyValuePlugin.interpret(&evidence)?;
let date = DateValuePlugin.interpret(&evidence)?;

let request = DerivationRequest {
    question: "How much was spent?".to_string(),
    operation: DerivationOperation::Sum,
    unit: None,
    operands: money
        .values
        .into_iter()
        .map(|mention| DerivationOperand::included(mention.ref_id, mention.value))
        .collect(),
};

let result = ValueOperationPlugin.derive(&request)?;
assert_eq!(result.answer.as_deref(), Some("USD 185"));
```

## Next Plugin Families

P0 next:

- unit and duration plugin: hours, days, weeks, quantities, rates;
- entity canonicalization plugin: same vendor/person/item across sessions;
- latest/current plugin: resolve superseded facts and active preferences;
- preference/state plugin: current likes, constraints, disallowed options;
- candidate-lifecycle helper: candidate, positive operand, negative operand,
  superseded candidate, context-only.

These remain above the kernel. The kernel should never learn what `sum`, money,
date math, or preference deduplication means.
