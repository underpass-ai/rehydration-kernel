#!/usr/bin/env python3
"""
Extract exhaustive E2E evaluation report from a run directory.

Usage:
  python3 scripts/ci/extract-e2e-report.py [run_dir]

  run_dir defaults to the latest artifacts/e2e-runs/*/ directory.

Reads:
  - results/*.json     (per-evaluation structured data)
  - test.log           (full stderr with inference responses and judge verdicts)

Produces:
  - <run_dir>/report-full.md   (exhaustive markdown with all tables and analysis)
"""

import json
import glob
import os
import sys
from collections import defaultdict
from pathlib import Path


def find_latest_run():
    base = Path(__file__).resolve().parent.parent.parent / "artifacts" / "e2e-runs"
    runs = sorted(base.glob("*/"), key=lambda p: p.name, reverse=True)
    if not runs:
        print("No run directories found in artifacts/e2e-runs/", file=sys.stderr)
        sys.exit(1)
    return runs[0]


def load_results(run_dir):
    files = sorted(glob.glob(str(run_dir / "results" / "*.json")))
    results = []
    for f in files:
        with open(f) as fh:
            d = json.load(fh)
            d["_file"] = os.path.basename(f)
            results.append(d)
    return results


def parse_variant(variant):
    """Parse 'micro-ops-explanatory-clean' into components."""
    parts = variant.split("-", 3)
    if len(parts) >= 4:
        return {"scale": parts[0], "domain": parts[1], "mix": parts[2], "noise": parts[3]}
    return {"scale": "?", "domain": "?", "mix": "?", "noise": "?"}


def parse_model(model):
    """Parse 'qwen3-8b→opus-4.6' into agent and judge."""
    if "\u2192" in model:
        parts = model.split("\u2192", 1)
        return parts[0], parts[1]
    return model, "?"


def ratio(ok, total):
    if total == 0:
        return "n/a"
    pct = 100.0 * ok / total
    return f"{ok}/{total} ({pct:.0f}%)"


def avg_latency(results):
    latencies = [r["latency_ms"] for r in results if r.get("latency_ms", 0) > 0]
    if not latencies:
        return "n/a"
    return f"{sum(latencies) / len(latencies):.0f}ms"


def fig_ref(run_dir, filename, caption=""):
    """Return markdown image reference if figure exists, else empty string."""
    fig_path = run_dir / "figures" / filename
    if fig_path.exists():
        alt = caption or filename.replace(".png", "").replace("_", " ")
        return f"\n![{alt}](figures/{filename})\n"
    return ""


def generate_report(run_dir, results):
    md = []
    md.append(f"# E2E Evaluation Matrix Report")
    md.append(f"")
    md.append(f"**Run**: `{run_dir.name}`")
    md.append(f"**Date**: {run_dir.name[:10]}")
    md.append(f"**Total evaluations**: {len(results)}")
    md.append(fig_ref(run_dir, "00_hypothesis_summary.png", "Hypothesis Validation Summary"))
    md.append(f"")

    # ── Overview ──
    total = len(results)
    task_ok = sum(1 for r in results if r.get("task") is True)
    task_fail = sum(1 for r in results if r.get("task") is False)
    task_err = sum(1 for r in results if r.get("task") is None)
    restart_ok = sum(1 for r in results if r.get("restart") is True)
    restart_exact = sum(1 for r in results if r.get("restart_exact") is True)
    restart_off1 = sum(1 for r in results if r.get("restart_off_by_one") is True)
    restart_competing = sum(1 for r in results if r.get("restart_on_competing") is True)
    restart_explained = sum(1 for r in results if r.get("restart_explained") is True)
    reason_ok = sum(1 for r in results if r.get("reason") is True)
    reason_correct_ok = sum(1 for r in results if r.get("reason_correct") is True)
    reason_distractor_ok = sum(1 for r in results if r.get("reason_distractor") is True)

    md.append("## Overview")
    md.append("")
    md.append("| Metric | OK | FAIL | ERR | Rate |")
    md.append("|--------|----|------|-----|------|")
    md.append(f"| Task | {task_ok} | {task_fail} | {task_err} | {ratio(task_ok, total)} |")
    md.append(f"| Restart | {restart_ok} | {total - restart_ok - task_err} | {task_err} | {ratio(restart_ok, total)} |")
    md.append(f"| ↳ Exact | {restart_exact} | | | {ratio(restart_exact, total)} |")
    md.append(f"| ↳ Off-by-one | {restart_off1} | | | {ratio(restart_off1, total)} |")
    md.append(f"| ↳ Competing branch | {restart_competing} | | | {ratio(restart_competing, total)} |")
    md.append(f"| ↳ Explained | {restart_explained} | | | {ratio(restart_explained, total)} |")
    md.append(f"| Reason Correct | {reason_correct_ok} | {total - reason_correct_ok - task_err} | {task_err} | {ratio(reason_correct_ok, total)} |")
    md.append(f"| Reason Distractor | {reason_distractor_ok} | | | {ratio(reason_distractor_ok, total)} |")
    md.append("")

    # ── By Agent x Judge x Prompt ──
    md.append("## By Agent x Judge x Prompt")
    md.append("")
    md.append("| Agent | Judge | Prompt | Task | Restart | Reason | Avg Latency |")
    md.append("|-------|-------|--------|------|---------|--------|-------------|")

    by_cell = defaultdict(list)
    for r in results:
        agent, judge = parse_model(r["model"])
        by_cell[(agent, judge, r["prompt"])].append(r)

    for (agent, judge, prompt), cell in sorted(by_cell.items()):
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        p = sum(1 for r in cell if r.get("reason") is True)
        lat = avg_latency(cell)
        md.append(f"| {agent} | {judge} | {prompt} | {ratio(t, n)} | {ratio(re, n)} | {ratio(p, n)} | {lat} |")

    md.append("")

    # ── By Agent (aggregated across judges and prompts) ──
    md.append("## By Agent (all judges, all prompts)")
    md.append("")
    md.append("| Agent | Task | Restart | Reason | Evals |")
    md.append("|-------|------|---------|--------|-------|")

    by_agent = defaultdict(list)
    for r in results:
        agent, _ = parse_model(r["model"])
        by_agent[agent].append(r)

    for agent, cell in sorted(by_agent.items()):
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        p = sum(1 for r in cell if r.get("reason") is True)
        md.append(f"| {agent} | {ratio(t, n)} | {ratio(re, n)} | {ratio(p, n)} | {n} |")

    md.append("")

    # ── By Judge (aggregated) ──
    md.append("## By Judge (all agents, all prompts)")
    md.append("")
    md.append("| Judge | Task | Restart | Reason | Evals |")
    md.append("|-------|------|---------|--------|-------|")

    by_judge = defaultdict(list)
    for r in results:
        _, judge = parse_model(r["model"])
        by_judge[judge].append(r)

    for judge, cell in sorted(by_judge.items()):
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        p = sum(1 for r in cell if r.get("reason") is True)
        md.append(f"| {judge} | {ratio(t, n)} | {ratio(re, n)} | {ratio(p, n)} | {n} |")

    md.append("")

    # ── By Prompt (aggregated) ──
    md.append("## By Prompt Variant")
    md.append(fig_ref(run_dir, "05_prompt_comparison.png", "Prompt Variant Effectiveness"))
    md.append("")
    md.append("| Prompt | Task | Restart | Reason | Evals |")
    md.append("|--------|------|---------|--------|-------|")

    by_prompt = defaultdict(list)
    for r in results:
        by_prompt[r["prompt"]].append(r)

    for prompt, cell in sorted(by_prompt.items()):
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        p = sum(1 for r in cell if r.get("reason") is True)
        md.append(f"| {prompt} | {ratio(t, n)} | {ratio(re, n)} | {ratio(p, n)} | {n} |")

    md.append("")

    # ── By Relation Mix (THE key signal) ──
    md.append("## By Relation Mix (key signal)")
    md.append(fig_ref(run_dir, "01_relation_mix_bars.png", "Rehydration Quality by Relation Type"))
    md.append("")
    md.append("| Mix | Task | Restart | Reason Correct | Reason Distractor | Evals |")
    md.append("|-----|------|---------|----------------|-------------------|-------|")

    by_mix = defaultdict(list)
    for r in results:
        v = parse_variant(r["variant"])
        by_mix[v["mix"]].append(r)

    for mix in ["explanatory", "structural", "mixed"]:
        cell = by_mix.get(mix, [])
        if not cell:
            continue
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        rc = sum(1 for r in cell if r.get("reason_correct") is True)
        rd = sum(1 for r in cell if r.get("reason_distractor") is True)
        md.append(f"| **{mix}** | **{ratio(t, n)}** | **{ratio(re, n)}** | **{ratio(rc, n)}** | **{ratio(rd, n)}** | {n} |")

    md.append("")

    # ── By Scale ──
    md.append("## By Scale")
    md.append(fig_ref(run_dir, "04_scale_effect.png", "Scale Effect"))
    md.append("")
    md.append("| Scale | Task | Restart | Reason | Evals |")
    md.append("|-------|------|---------|--------|-------|")

    by_scale = defaultdict(list)
    for r in results:
        v = parse_variant(r["variant"])
        by_scale[v["scale"]].append(r)

    for scale in ["micro", "meso", "stress"]:
        cell = by_scale.get(scale, [])
        if not cell:
            continue
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        p = sum(1 for r in cell if r.get("reason") is True)
        md.append(f"| {scale} | {ratio(t, n)} | {ratio(re, n)} | {ratio(p, n)} | {n} |")

    md.append("")

    # ── By Domain ──
    md.append("## By Domain")
    md.append("")
    md.append("| Domain | Task | Restart | Reason | Evals |")
    md.append("|--------|------|---------|--------|-------|")

    by_domain = defaultdict(list)
    for r in results:
        v = parse_variant(r["variant"])
        by_domain[v["domain"]].append(r)

    for domain, cell in sorted(by_domain.items()):
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        p = sum(1 for r in cell if r.get("reason") is True)
        md.append(f"| {domain} | {ratio(t, n)} | {ratio(re, n)} | {ratio(p, n)} | {n} |")

    md.append("")

    # ── By Noise Mode ──
    md.append("## By Noise Mode")
    md.append(fig_ref(run_dir, "07_noise_impact.png", "Noise Impact"))
    md.append("")
    md.append("| Noise | Task | Restart | Reason Correct | Reason Distractor | Evals |")
    md.append("|-------|------|---------|----------------|-------------------|-------|")

    by_noise = defaultdict(list)
    for r in results:
        v = parse_variant(r["variant"])
        by_noise[v["noise"]].append(r)

    for noise in ["clean", "competing", "conflicting", "restart"]:
        cell = by_noise.get(noise, [])
        if not cell:
            continue
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        rc = sum(1 for r in cell if r.get("reason_correct") is True)
        rd = sum(1 for r in cell if r.get("reason_distractor") is True)
        md.append(f"| **{noise}** | {ratio(t, n)} | {ratio(re, n)} | {ratio(rc, n)} | {ratio(rd, n)} | {n} |")

    # Any noise modes not in the predefined list
    for noise, cell in sorted(by_noise.items()):
        if noise in ("clean", "competing", "conflicting", "restart"):
            continue
        n = len(cell)
        t = sum(1 for r in cell if r.get("task") is True)
        re = sum(1 for r in cell if r.get("restart") is True)
        rc = sum(1 for r in cell if r.get("reason_correct") is True)
        rd = sum(1 for r in cell if r.get("reason_distractor") is True)
        md.append(f"| {noise} | {ratio(t, n)} | {ratio(re, n)} | {ratio(rc, n)} | {ratio(rd, n)} | {n} |")

    md.append("")

    # ── Cross: Noise x Mix ──
    md.append("## Noise x Relation Mix (Task)")
    md.append("")
    md.append("| Noise | Explanatory | Structural | Mixed |")
    md.append("|-------|-------------|------------|-------|")

    for noise in ["clean", "competing", "conflicting", "restart"]:
        cell = by_noise.get(noise, [])
        if not cell:
            continue
        row = [noise]
        for mix in ["explanatory", "structural", "mixed"]:
            mix_cell = [r for r in cell if parse_variant(r["variant"])["mix"] == mix]
            n = len(mix_cell)
            t = sum(1 for r in mix_cell if r.get("task") is True)
            row.append(ratio(t, n))
        md.append(f"| {' | '.join(row)} |")

    md.append("")

    # ── Cross: Agent x Mix (most important table) ──
    md.append("## Agent x Relation Mix (Task)")
    md.append(fig_ref(run_dir, "02_agent_x_mix_task.png", "Task Identification by Agent and Relation Type"))
    md.append("")
    md.append("| Agent | Explanatory | Structural | Mixed |")
    md.append("|-------|-------------|------------|-------|")

    for agent in sorted(by_agent.keys()):
        row = [agent]
        for mix in ["explanatory", "structural", "mixed"]:
            cell = [r for r in by_agent[agent] if parse_variant(r["variant"])["mix"] == mix]
            n = len(cell)
            t = sum(1 for r in cell if r.get("task") is True)
            row.append(ratio(t, n))
        md.append(f"| {' | '.join(row)} |")

    md.append("")

    # ── Cross: Agent x Mix (Reason Correct) ──
    md.append("## Agent x Relation Mix (Reason Correct)")
    md.append(fig_ref(run_dir, "03_agent_x_mix_reason.png", "Rationale Preservation by Agent and Relation Type"))
    md.append("")
    md.append("| Agent | Explanatory | Structural | Mixed |")
    md.append("|-------|-------------|------------|-------|")

    for agent in sorted(by_agent.keys()):
        row = [agent]
        for mix in ["explanatory", "structural", "mixed"]:
            cell = [r for r in by_agent[agent] if parse_variant(r["variant"])["mix"] == mix]
            n = len(cell)
            p = sum(1 for r in cell if r.get("reason_correct") is True)
            row.append(ratio(p, n))
        md.append(f"| {' | '.join(row)} |")

    md.append("")

    # ── Cross: Agent x Mix (Reason Distractor) ──
    md.append("## Agent x Relation Mix (Reason Distractor)")
    md.append("")
    md.append("| Agent | Explanatory | Structural | Mixed |")
    md.append("|-------|-------------|------------|-------|")

    for agent in sorted(by_agent.keys()):
        row = [agent]
        for mix in ["explanatory", "structural", "mixed"]:
            cell = [r for r in by_agent[agent] if parse_variant(r["variant"])["mix"] == mix]
            n = len(cell)
            d = sum(1 for r in cell if r.get("reason_distractor") is True)
            row.append(ratio(d, n))
        md.append(f"| {' | '.join(row)} |")

    md.append("")

    # ── Controlled comparison: Agent (fixed judge) ──
    md.append("## Controlled: Agent Comparison (fixed judge)")
    md.append("")
    md.append("Each table holds the judge constant, enabling fair agent comparison.")
    md.append("Evals per cell are equal when judge is fixed.")
    md.append("")

    for judge in sorted(by_judge.keys()):
        judge_results = [r for r in results if parse_model(r["model"])[1] == judge]
        if not judge_results:
            continue
        agents_in_judge = sorted(set(parse_model(r["model"])[0] for r in judge_results))
        md.append(f"### Judge = {judge}")
        md.append("")
        md.append("| Agent | Evals | Task | Restart | Reason Correct | Reason Distractor |")
        md.append("|-------|-------|------|---------|----------------|-------------------|")
        for agent in agents_in_judge:
            cell = [r for r in judge_results if parse_model(r["model"])[0] == agent]
            n = len(cell)
            t = sum(1 for r in cell if r.get("task") is True)
            re = sum(1 for r in cell if r.get("restart") is True)
            rc = sum(1 for r in cell if r.get("reason_correct") is True)
            rd = sum(1 for r in cell if r.get("reason_distractor") is True)
            md.append(f"| {agent} | {n} | {ratio(t, n)} | {ratio(re, n)} | {ratio(rc, n)} | {ratio(rd, n)} |")
        md.append("")

    # ── Controlled comparison: Judge (fixed agent) ──
    md.append("## Controlled: Judge Comparison (fixed agent)")
    md.append("")
    md.append("Each table holds the agent constant, enabling fair judge comparison.")
    md.append("")

    for agent in sorted(by_agent.keys()):
        agent_results = [r for r in results if parse_model(r["model"])[0] == agent]
        if not agent_results:
            continue
        judges_in_agent = sorted(set(parse_model(r["model"])[1] for r in agent_results))
        if len(judges_in_agent) < 2:
            continue
        md.append(f"### Agent = {agent}")
        md.append("")
        md.append("| Judge | Evals | Task | Restart | Reason Correct | Reason Distractor |")
        md.append("|-------|-------|------|---------|----------------|-------------------|")
        for judge in judges_in_agent:
            cell = [r for r in agent_results if parse_model(r["model"])[1] == judge]
            n = len(cell)
            t = sum(1 for r in cell if r.get("task") is True)
            re = sum(1 for r in cell if r.get("restart") is True)
            rc = sum(1 for r in cell if r.get("reason_correct") is True)
            rd = sum(1 for r in cell if r.get("reason_distractor") is True)
            md.append(f"| {judge} | {n} | {ratio(t, n)} | {ratio(re, n)} | {ratio(rc, n)} | {ratio(rd, n)} |")
        md.append("")

    # ── Cross: Scale x Mix (Task) ──
    md.append("## Scale x Relation Mix (Task)")
    md.append(fig_ref(run_dir, "04_scale_effect.png", "Scale Effect on Task Identification"))
    md.append("")
    md.append("| Scale | Explanatory | Structural | Mixed |")
    md.append("|-------|-------------|------------|-------|")

    for scale in ["micro", "meso", "stress"]:
        row = [scale]
        for mix in ["explanatory", "structural", "mixed"]:
            cell = [r for r in results
                    if parse_variant(r["variant"])["scale"] == scale
                    and parse_variant(r["variant"])["mix"] == mix]
            n = len(cell)
            t = sum(1 for r in cell if r.get("task") is True)
            row.append(ratio(t, n))
        md.append(f"| {' | '.join(row)} |")

    md.append("")

    # ── Prompt convergence: strict vs lenient ──
    md.append("## Prompt Convergence: Strict vs Lenient")
    md.append(fig_ref(run_dir, "09_convergence_heatmap.png", "Convergence Heatmap"))
    md.append("")
    md.append("| Agent→Judge | Metric | Strict | Lenient | Default | Delta S-L |")
    md.append("|-------------|--------|--------|---------|---------|-----------|")

    for (agent, judge) in sorted(set((a, j) for (a, j, _) in by_cell.keys())):
        for metric_name, metric_key in [("Task", "task"), ("Restart", "restart"), ("Reason", "reason")]:
            vals = {}
            for prompt in ["strict-judge", "lenient-judge", "default"]:
                cell = by_cell.get((agent, judge, prompt), [])
                n = len(cell)
                ok = sum(1 for r in cell if r.get(metric_key) is True)
                vals[prompt] = (ok, n)
            s_ok, s_n = vals.get("strict-judge", (0, 0))
            l_ok, l_n = vals.get("lenient-judge", (0, 0))
            d_ok, d_n = vals.get("default", (0, 0))
            delta = abs(s_ok - l_ok) if s_n > 0 and l_n > 0 else "?"
            md.append(f"| {agent}\u2192{judge} | {metric_name} | {ratio(s_ok, s_n)} | {ratio(l_ok, l_n)} | {ratio(d_ok, d_n)} | {delta} |")

    md.append("")

    # ── Latency by agent ──
    md.append("## Latency by Agent")
    md.append(fig_ref(run_dir, "08_latency_boxplot.png", "Latency Distribution"))
    md.append("")
    md.append("| Agent | Avg Latency | Min | Max | P50 |")
    md.append("|-------|-------------|-----|-----|-----|")

    for agent, cell in sorted(by_agent.items()):
        latencies = sorted([r["latency_ms"] for r in cell if r.get("latency_ms", 0) > 0])
        if not latencies:
            continue
        avg = sum(latencies) / len(latencies)
        p50 = latencies[len(latencies) // 2]
        md.append(f"| {agent} | {avg:.0f}ms | {latencies[0]:.0f}ms | {latencies[-1]:.0f}ms | {p50:.0f}ms |")

    md.append("")

    # ── Cost estimate ──
    md.append("## Cost Estimate")
    md.append("")

    prices = {
        "qwen3-8b": {"input": 0, "output": 0},
        "gpt-5.4": {"input": 2.50, "output": 10.0},
        "opus-4.6": {"input": 15.0, "output": 75.0},
    }
    input_per_call = 700
    total_cost = 0.0

    md.append("| Role | Model | Evals | Est. Cost |")
    md.append("|------|-------|-------|-----------|")

    for agent, cell in sorted(by_agent.items()):
        n = len(cell)
        out_tokens = sum(len(r.get("agent_response", "")) // 4 for r in cell)
        p = prices.get(agent, {"input": 0, "output": 0})
        cost = (n * input_per_call * p["input"] + out_tokens * p["output"]) / 1_000_000
        total_cost += cost
        md.append(f"| agent | {agent} | {n} | ${cost:.2f} |")

    for judge, cell in sorted(by_judge.items()):
        n = len(cell)
        out_tokens = sum(len(r.get("judge_raw", "") or "") // 4 for r in cell)
        p = prices.get(judge, {"input": 0, "output": 0})
        cost = (n * 600 * p["input"] + out_tokens * p["output"]) / 1_000_000
        total_cost += cost
        md.append(f"| judge | {judge} | {n} | ${cost:.2f} |")

    md.append(f"| **total** | | **{len(results)}** | **${total_cost:.2f}** |")
    md.append("")

    # ── Restart diagnostic by noise mode ──
    md.append("## Restart Diagnostic by Noise Mode")
    md.append("")
    md.append("| Noise | Restart | Exact | Off-by-1 | Competing | Explained | Evals |")
    md.append("|-------|---------|-------|----------|-----------|-----------|-------|")

    for noise in ["clean", "competing", "conflicting", "restart"]:
        cell = by_noise.get(noise, [])
        if not cell:
            continue
        n = len(cell)
        re = sum(1 for r in cell if r.get("restart") is True)
        rx = sum(1 for r in cell if r.get("restart_exact") is True)
        ro = sum(1 for r in cell if r.get("restart_off_by_one") is True)
        rcb = sum(1 for r in cell if r.get("restart_on_competing") is True)
        rexp = sum(1 for r in cell if r.get("restart_explained") is True)
        md.append(f"| **{noise}** | {ratio(re, n)} | {ratio(rx, n)} | {ratio(ro, n)} | {ratio(rcb, n)} | {ratio(rexp, n)} | {n} |")

    md.append("")

    # ── Statistical summary per dimension ──
    md.append("## Statistical Summary")
    md.append(fig_ref(run_dir, "10_judge_strictness.png", "Judge Strictness"))
    md.append(fig_ref(run_dir, "15_inter_rater_agreement.png", "Inter-Rater Agreement"))
    md.append("")
    md.append("Success rates with 95% confidence intervals (Wilson score).")
    md.append("")

    def wilson_ci(ok, n):
        """Wilson score interval for binomial proportion."""
        if n == 0:
            return 0.0, 0.0, 0.0
        import math
        z = 1.96
        p_hat = ok / n
        denom = 1 + z * z / n
        center = (p_hat + z * z / (2 * n)) / denom
        spread = z * math.sqrt((p_hat * (1 - p_hat) + z * z / (4 * n)) / n) / denom
        lo = max(0.0, center - spread)
        hi = min(1.0, center + spread)
        return p_hat, lo, hi

    md.append("### Task Success Rate by Relation Mix")
    md.append("")
    md.append("| Mix | N | Rate | 95% CI |")
    md.append("|-----|---|------|--------|")
    for mix in ["explanatory", "structural", "mixed"]:
        cell = by_mix.get(mix, [])
        n = len(cell)
        ok = sum(1 for r in cell if r.get("task") is True)
        rate, lo, hi = wilson_ci(ok, n)
        md.append(f"| {mix} | {n} | {rate:.1%} | [{lo:.1%}, {hi:.1%}] |")
    md.append("")

    md.append("### Reason Correct (Main Path) Rate by Relation Mix")
    md.append("")
    md.append("| Mix | N | Rate | 95% CI |")
    md.append("|-----|---|------|--------|")
    for mix in ["explanatory", "structural", "mixed"]:
        cell = by_mix.get(mix, [])
        n = len(cell)
        ok = sum(1 for r in cell if r.get("reason_correct") is True)
        rate, lo, hi = wilson_ci(ok, n)
        md.append(f"| {mix} | {n} | {rate:.1%} | [{lo:.1%}, {hi:.1%}] |")
    md.append("")

    md.append("### Reason Distractor Leakage Rate by Relation Mix")
    md.append("")
    md.append("| Mix | N | Rate | 95% CI |")
    md.append("|-----|---|------|--------|")
    for mix in ["explanatory", "structural", "mixed"]:
        cell = by_mix.get(mix, [])
        n = len(cell)
        ok = sum(1 for r in cell if r.get("reason_distractor") is True)
        rate, lo, hi = wilson_ci(ok, n)
        md.append(f"| {mix} | {n} | {rate:.1%} | [{lo:.1%}, {hi:.1%}] |")
    md.append("")

    md.append("### Task Success Rate by Agent")
    md.append("")
    md.append("| Agent | N | Rate | 95% CI |")
    md.append("|-------|---|------|--------|")
    for agent in sorted(by_agent.keys()):
        cell = by_agent[agent]
        n = len(cell)
        ok = sum(1 for r in cell if r.get("task") is True)
        rate, lo, hi = wilson_ci(ok, n)
        md.append(f"| {agent} | {n} | {rate:.1%} | [{lo:.1%}, {hi:.1%}] |")
    md.append("")

    md.append("### Latency Distribution by Agent (ms)")
    md.append("")
    md.append("| Agent | N | Mean | Std | P25 | P50 | P75 | P95 |")
    md.append("|-------|---|------|-----|-----|-----|-----|-----|")
    for agent in sorted(by_agent.keys()):
        latencies = sorted([r["latency_ms"] for r in by_agent[agent] if r.get("latency_ms", 0) > 0])
        if not latencies:
            continue
        import math
        n = len(latencies)
        mean = sum(latencies) / n
        std = math.sqrt(sum((x - mean) ** 2 for x in latencies) / n)
        p25 = latencies[int(n * 0.25)]
        p50 = latencies[int(n * 0.50)]
        p75 = latencies[int(n * 0.75)]
        p95 = latencies[int(n * 0.95)]
        md.append(f"| {agent} | {n} | {mean:.0f} | {std:.0f} | {p25:.0f} | {p50:.0f} | {p75:.0f} | {p95:.0f} |")
    md.append("")

    # ── Export CSV ──
    csv_path = run_dir / "results.csv"
    with open(csv_path, "w") as f:
        f.write("eval_num,agent,judge,prompt,variant,scale,domain,mix,noise,task,restart,restart_exact,restart_off_by_one,restart_on_competing,restart_explained,reason,reason_correct,reason_distractor,latency_ms,failure_point,restart_node\n")
        for i, r in enumerate(results, 1):
            agent, judge = parse_model(r["model"])
            v = parse_variant(r["variant"])
            t = "true" if r.get("task") else ("false" if r.get("task") is False else "null")
            re = "true" if r.get("restart") else ("false" if r.get("restart") is False else "null")
            p = "true" if r.get("reason") else ("false" if r.get("reason") is False else "null")
            rx = "true" if r.get("restart_exact") else ("false" if r.get("restart_exact") is False else "null")
            ro = "true" if r.get("restart_off_by_one") else ("false" if r.get("restart_off_by_one") is False else "null")
            rcb = "true" if r.get("restart_on_competing") else ("false" if r.get("restart_on_competing") is False else "null")
            rexp = "true" if r.get("restart_explained") else ("false" if r.get("restart_explained") is False else "null")
            rc = "true" if r.get("reason_correct") else ("false" if r.get("reason_correct") is False else "null")
            rd = "true" if r.get("reason_distractor") else ("false" if r.get("reason_distractor") is False else "null")
            lat = f"{r.get('latency_ms', 0):.1f}"
            fp = ""
            rn = ""
            try:
                parsed = json.loads(r.get("agent_response", "").strip())
                fp = parsed.get("failure_point", "").replace('"', '""')
                rn = parsed.get("restart_node", "").replace('"', '""')
            except Exception:
                pass
            f.write(f'{i},"{agent}","{judge}","{r["prompt"]}","{r["variant"]}","{v["scale"]}","{v["domain"]}","{v["mix"]}","{v["noise"]}",{t},{re},{rx},{ro},{rcb},{rexp},{p},{rc},{rd},{lat},"{fp}","{rn}"\n')

    md.append("## Kernel Metrics")
    md.append(fig_ref(run_dir, "11_kernel_token_efficiency.png", "Kernel Token Efficiency"))
    md.append(fig_ref(run_dir, "12_kernel_causal_signal.png", "Causal Signal in Rendered Context"))
    md.append(fig_ref(run_dir, "14_causal_depth_reached.png", "Causal Depth Reached by Agent"))
    md.append(fig_ref(run_dir, "16_chain_depth_vs_task.png", "Chain Depth vs Task Success"))
    md.append(fig_ref(run_dir, "13_response_parsing.png", "Response Parsing Reliability"))
    md.append(fig_ref(run_dir, "06_judge_bias.png", "Judge Bias"))
    md.append("")
    md.append(f"## Data Export")
    md.append(f"")
    md.append(f"CSV with all 720 evaluations: `results.csv`")
    md.append(f"")

    # ── Every evaluation line-by-line ──
    md.append("## All Evaluations (line by line)")
    md.append("")
    md.append("| # | Agent | Judge | Prompt | Variant | Task | Restart | Reason | Rc | Rd | Latency | Failure Point | Restart Node |")
    md.append("|---|-------|-------|--------|---------|------|---------|--------|----|----|---------|---------------|--------------|")

    for i, r in enumerate(results, 1):
        agent, judge = parse_model(r["model"])
        t = "OK" if r.get("task") else ("FAIL" if r.get("task") is False else "ERR")
        re = "OK" if r.get("restart") else ("FAIL" if r.get("restart") is False else "ERR")
        p = "OK" if r.get("reason") else ("FAIL" if r.get("reason") is False else "ERR")
        lat = f"{r.get('latency_ms', 0):.0f}ms"

        # Parse agent response for failure_point and restart_node
        fp = "?"
        rn = "?"
        try:
            resp = r.get("agent_response", "")
            parsed = json.loads(resp.strip())
            fp = parsed.get("failure_point", "?")[:60]
            rn = parsed.get("restart_node", "?")[:60]
        except Exception:
            fp = "(parse error)"
            rn = "(parse error)"

        rc = "OK" if r.get("reason_correct") else ("FAIL" if r.get("reason_correct") is False else "-")
        rd = "LEAK" if r.get("reason_distractor") else ("-" if r.get("reason_distractor") is False else "-")
        md.append(f"| {i} | {agent} | {judge} | {r['prompt']} | {r['variant']} | {t} | {re} | {p} | {rc} | {rd} | {lat} | {fp} | {rn} |")

    md.append("")
    md.append(f"---\n\nGenerated from `{run_dir.name}` by `scripts/ci/extract-e2e-report.py`")

    return "\n".join(md)


def main():
    if len(sys.argv) > 1:
        run_dir = Path(sys.argv[1])
    else:
        run_dir = find_latest_run()

    print(f"Extracting report from: {run_dir}", file=sys.stderr)

    results = load_results(run_dir)
    if not results:
        print(f"No results found in {run_dir}/results/", file=sys.stderr)
        sys.exit(1)

    print(f"Loaded {len(results)} evaluations", file=sys.stderr)

    report = generate_report(run_dir, results)

    output_path = run_dir / "report-full.md"
    with open(output_path, "w") as f:
        f.write(report)

    print(f"Report written to: {output_path}", file=sys.stderr)
    print(f"  {len(results)} evaluations, {len(report.splitlines())} lines", file=sys.stderr)


if __name__ == "__main__":
    main()
