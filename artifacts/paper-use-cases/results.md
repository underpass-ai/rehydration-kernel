# Paper Use Case Results

Source: `artifacts/paper-use-cases/summary.json`

## Metrics Table

| Use case | Variant | Relation mode | Detail mode | Scale | Budget | Explanation fidelity | Detail fidelity | Causal score | Retry hit | Retry score | Tokens |
| --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| uc1_failure_diagnosis_rehydration | detail_only_with_detail | detail_only | with_detail | micro | 4096 | 0.0 | 1.0 | 0.42857142857142855 |  | 0.0 | 113 |
| uc1_failure_diagnosis_rehydration | full_explanatory_with_detail | explanatory | with_detail | micro | 4096 | 1.0 | 1.0 | 1.0 | true | 1.0 | 135 |
| uc1_failure_diagnosis_rehydration | full_explanatory_with_detail__meso | explanatory | with_detail | meso | 4096 | 1.0 | 1.0 | 1.0 | true | 1.0 | 135 |
| uc1_failure_diagnosis_rehydration | full_explanatory_without_detail | explanatory | without_detail | micro | 4096 | 1.0 | 0.0 | 0.8571428571428571 | true | 1.0 | 116 |
| uc1_failure_diagnosis_rehydration | structural_only_with_detail | structural_only | with_detail | micro | 4096 | 0.0 | 1.0 | 0.14285714285714285 |  | 0.0 | 97 |
| uc2_why_implementation_trace | detail_only_with_detail | detail_only | with_detail | micro | 4096 | 0.0 | 1.0 | 0.42857142857142855 |  |  | 111 |
| uc2_why_implementation_trace | full_explanatory_with_detail | explanatory | with_detail | micro | 4096 | 1.0 | 1.0 | 1.0 |  |  | 126 |
| uc2_why_implementation_trace | full_explanatory_without_detail | explanatory | without_detail | micro | 4096 | 1.0 | 0.0 | 0.8571428571428571 |  |  | 111 |
| uc2_why_implementation_trace | structural_only_with_detail | structural_only | with_detail | micro | 4096 | 0.0 | 1.0 | 0.14285714285714285 |  |  | 91 |
| uc3_interrupted_handoff_resume | detail_only_with_detail | detail_only | with_detail | micro | 4096 | 0.0 | 1.0 | 0.42857142857142855 |  | 0.0 | 208 |
| uc3_interrupted_handoff_resume | full_explanatory_with_detail | explanatory | with_detail | micro | 4096 | 1.0 | 1.0 | 1.0 | true | 1.0 | 260 |
| uc3_interrupted_handoff_resume | full_explanatory_without_detail | explanatory | without_detail | micro | 4096 | 1.0 | 0.0 | 0.8571428571428571 | true | 1.0 | 234 |
| uc3_interrupted_handoff_resume | structural_only_with_detail | structural_only | with_detail | micro | 4096 | 0.0 | 1.0 | 0.14285714285714285 |  | 0.0 | 169 |
| uc4_constraint_reason_under_token_pressure | full_explanatory_with_detail | explanatory | with_detail | micro | 4096 | 1.0 | 1.0 | 1.0 |  |  | 122 |
| uc4_constraint_reason_under_token_pressure | full_explanatory_with_detail__budget_96 | explanatory | with_detail | micro | 96 | 1.0 | 0.0 | 1.0 |  |  | 89 |
| uc4_constraint_reason_under_token_pressure | structural_only_with_detail__budget_96 | structural_only | with_detail | micro | 96 | 0.0 | 0.0 | 0.125 |  |  | 64 |

## Key Findings

- uc1_failure_diagnosis_rehydration: `detail_only_with_detail` reaches explanation fidelity `0.0`, causal score `0.42857142857142855`, retry score `0.0`, and renders `113` tokens under budget `4096`.
- uc1_failure_diagnosis_rehydration: `full_explanatory_with_detail` reaches explanation fidelity `1.0`, causal score `1.0`, retry score `1.0`, and renders `135` tokens under budget `4096`.
- uc1_failure_diagnosis_rehydration: `full_explanatory_with_detail__meso` reaches explanation fidelity `1.0`, causal score `1.0`, retry score `1.0`, and renders `135` tokens under budget `4096`.
- uc1_failure_diagnosis_rehydration: `full_explanatory_without_detail` reaches explanation fidelity `1.0`, causal score `0.8571428571428571`, retry score `1.0`, and renders `116` tokens under budget `4096`.
- uc1_failure_diagnosis_rehydration: `structural_only_with_detail` reaches explanation fidelity `0.0`, causal score `0.14285714285714285`, retry score `0.0`, and renders `97` tokens under budget `4096`.
- uc2_why_implementation_trace: `detail_only_with_detail` reaches explanation fidelity `0.0`, causal score `0.42857142857142855`, retry score `n/a`, and renders `111` tokens under budget `4096`.
- uc2_why_implementation_trace: `full_explanatory_with_detail` reaches explanation fidelity `1.0`, causal score `1.0`, retry score `n/a`, and renders `126` tokens under budget `4096`.
- uc2_why_implementation_trace: `full_explanatory_without_detail` reaches explanation fidelity `1.0`, causal score `0.8571428571428571`, retry score `n/a`, and renders `111` tokens under budget `4096`.
- uc2_why_implementation_trace: `structural_only_with_detail` reaches explanation fidelity `0.0`, causal score `0.14285714285714285`, retry score `n/a`, and renders `91` tokens under budget `4096`.
- uc3_interrupted_handoff_resume: `detail_only_with_detail` reaches explanation fidelity `0.0`, causal score `0.42857142857142855`, retry score `0.0`, and renders `208` tokens under budget `4096`.
- uc3_interrupted_handoff_resume: `full_explanatory_with_detail` reaches explanation fidelity `1.0`, causal score `1.0`, retry score `1.0`, and renders `260` tokens under budget `4096`.
- uc3_interrupted_handoff_resume: `full_explanatory_without_detail` reaches explanation fidelity `1.0`, causal score `0.8571428571428571`, retry score `1.0`, and renders `234` tokens under budget `4096`.
- uc3_interrupted_handoff_resume: `structural_only_with_detail` reaches explanation fidelity `0.0`, causal score `0.14285714285714285`, retry score `0.0`, and renders `169` tokens under budget `4096`.
- uc4_constraint_reason_under_token_pressure: `full_explanatory_with_detail` reaches explanation fidelity `1.0`, causal score `1.0`, retry score `n/a`, and renders `122` tokens under budget `4096`.
- uc4_constraint_reason_under_token_pressure: `full_explanatory_with_detail__budget_96` reaches explanation fidelity `1.0`, causal score `1.0`, retry score `n/a`, and renders `89` tokens under budget `96`.
- uc4_constraint_reason_under_token_pressure: `structural_only_with_detail__budget_96` reaches explanation fidelity `0.0`, causal score `0.125`, retry score `n/a`, and renders `64` tokens under budget `96`.
