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
- normalize currency, dates, units, and entities;
- run deterministic operations over typed operands;
- return answer, value, refs, excluded refs, and diagnostics.

Currency and dates are treated as small domains, not loose scalar types. Plugins
must fail fast on incompatible operands. For example, a currency sum cannot
silently mix `USD` and `EUR`; a date sum is invalid; a date difference returns a
typed number in days with source refs.

## Reader And Writer Use

The domain plugins are shared code. They are not owned by the reader or the
writer.

Writer-side use:

- parse incoming memories for typed mentions such as currency amounts and dates;
- attach typed metadata or derived value nodes with provenance;
- build search/index fields for later retrieval;
- reject malformed explicit domain values early.

Reader-side use:

- consume kernel evidence refs;
- let a reader or agent decide which mentions are operands for the current
  question;
- compute query-time derivations such as monetary totals or elapsed days;
- return a proof object with included and excluded refs.

The writer must not invent question-dependent operations. A dollar amount found
in a memory is only a typed currency mention. It becomes a sum operand only when
a reader, agent, or explicit upstream event marks it as such.

## Implemented Slice

The first implementation lives in
`crates/rehydration-testkit/src/interpretation_plugins.rs` and is exported from
`rehydration-testkit`.

Text normalization lives in
`crates/rehydration-testkit/src/text_normalization.rs`. The first cut is only a
deterministic span segmenter:

- normal text spans;
- protected code spans, including fenced and inline Markdown code;
- protected math spans, including LaTeX-style inline/block delimiters;
- protected URL spans;
- normalized text for comparison while preserving original byte offsets.

Money, date, and scalar-value extraction remain separate plugins. They should
consume this segmentation in a later cut instead of each plugin inventing its
own protected-text rules.

Implemented domain plugins:

- `CurrencyDerivationPlugin`: owns currency extraction plus monetary
  operations. It normalizes values into `InterpretedValue::Money` with ISO-like
  currency code, decimal amount, and minor units.
- `DateDerivationPlugin`: owns date extraction plus temporal operations. It
  normalizes values into `InterpretedValue::Date` and supports temporal
  difference, latest/max-by, list, and explicit abstention.

Lower-level primitives:

- `MoneyValuePlugin`: extracts money mentions such as `$1,200.50`, `EUR 30`,
  `30 dollars`, and normalizes them into `InterpretedValue::Money`.
- `DateValuePlugin`: extracts ISO-like dates and simple named dates such as
  `2026-05-06`, `05/06/2026`, and `May 6, 2026`.
- `ValueOperationPlugin`: shared deterministic primitive used by domain
  plugins. It is not the product-facing plugin boundary.

The operation input is deliberately separate from extraction. A money mention is
not automatically a sum operand. The reader or agent must create a derivation
request that labels each candidate as `include`, `exclude`, or `context`.

## Minimal Flow

1. Ask the kernel for context and keep the refs.
2. Run value extractors on the retrieved evidence.
3. Let a reader or agent select operands against the question.
4. Run the matching domain plugin, for example `CurrencyDerivationPlugin` or
   `DateDerivationPlugin`.
5. Optionally write the derived result back to memory with provenance
   `derived_from: [refs...]`.

Example shape:

```rust
use rehydration_testkit::{
    CurrencyDerivationPlugin, DerivationOperand, DerivationOperation,
    DerivationRequest, EvidenceFragment, EvidenceInterpretationInput,
};

let evidence = EvidenceInterpretationInput::new(vec![EvidenceFragment::new(
    "turn:42",
    "Paid $120 on 2026-05-01 and $65 on 2026-05-06.",
)]);

let currency = CurrencyDerivationPlugin;
let money = currency.interpret(&evidence)?;

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

let result = currency.derive(&request)?;
assert_eq!(result.answer.as_deref(), Some("USD 185"));
```

## Next Plugin Families

P0 next:

- counting plugin: counted entities, dedupe keys, quantity-vs-cardinality
  separation;
- unit and duration plugin: hours, days, weeks, quantities, rates;
- entity canonicalization plugin: same vendor/person/item across sessions;
- latest/current plugin: resolve superseded facts and active preferences;
- preference/state plugin: current likes, constraints, disallowed options;
- candidate-lifecycle helper: candidate, positive operand, negative operand,
  superseded candidate, context-only.

These remain above the kernel. The kernel should never learn what currency
arithmetic, date math, counting, or preference deduplication means.
