#!/usr/bin/env python3
"""
Generate publication-quality figures from E2E evaluation results.

Usage:
  python3 scripts/ci/extract-e2e-figures.py [run_dir]

Reads results/*.json, produces PNG figures in <run_dir>/figures/.
"""

import json
import glob
import os
import sys
from collections import defaultdict
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
import numpy as np


# ── Style ──

COLORS = {
    "explanatory": "#2ecc71",
    "structural": "#e74c3c",
    "mixed": "#3498db",
    "qwen3-8b": "#9b59b6",
    "gpt-5.4": "#e67e22",
    "opus-4.6": "#2c3e50",
    "micro": "#1abc9c",
    "meso": "#f39c12",
    "stress": "#e74c3c",
    "clean": "#3498db",
    "competing": "#e74c3c",
}

def setup_style():
    plt.rcParams.update({
        "figure.facecolor": "white",
        "axes.facecolor": "white",
        "axes.grid": True,
        "grid.alpha": 0.3,
        "font.size": 11,
        "axes.titlesize": 13,
        "axes.labelsize": 11,
        "figure.dpi": 150,
    })


# ── Data loading ──

def find_latest_run():
    base = Path(__file__).resolve().parent.parent.parent / "artifacts" / "e2e-runs"
    runs = sorted(base.glob("*/"), key=lambda p: p.name, reverse=True)
    if not runs:
        print("No run directories found", file=sys.stderr)
        sys.exit(1)
    return runs[0]


def load_results(run_dir):
    files = sorted(glob.glob(str(run_dir / "results" / "*.json")))
    results = []
    for f in files:
        with open(f) as fh:
            d = json.load(fh)
            results.append(d)
    return results


def parse_variant(variant):
    parts = variant.split("-", 3)
    if len(parts) >= 4:
        return {"scale": parts[0], "domain": parts[1], "mix": parts[2], "noise": parts[3]}
    return {"scale": "?", "domain": "?", "mix": "?", "noise": "?"}


def parse_model(model):
    if "\u2192" in model:
        parts = model.split("\u2192", 1)
        return parts[0], parts[1]
    return model, "?"


def rate(results, key):
    n = len(results)
    if n == 0:
        return 0.0
    return sum(1 for r in results if r.get(key) is True) / n


def wilson_ci(ok, n):
    if n == 0:
        return 0.0, 0.0, 0.0
    import math
    z = 1.96
    p_hat = ok / n
    denom = 1 + z * z / n
    center = (p_hat + z * z / (2 * n)) / denom
    spread = z * math.sqrt((p_hat * (1 - p_hat) + z * z / (4 * n)) / n) / denom
    return p_hat, max(0, center - spread), min(1, center + spread)


# ── Figures ──

def fig_relation_mix_bars(results, fig_dir):
    """Bar chart: Task + Reason by relation mix — the key signal."""
    by_mix = defaultdict(list)
    for r in results:
        v = parse_variant(r["variant"])
        by_mix[v["mix"]].append(r)

    mixes = ["explanatory", "mixed", "structural"]
    metrics = ["task", "reason"]
    labels = ["Task (failure identification)", "Reason (rationale preservation)"]

    fig, ax = plt.subplots(figsize=(8, 5))

    x = np.arange(len(mixes))
    width = 0.35

    for i, (metric, label) in enumerate(zip(metrics, labels)):
        rates = []
        errs_lo = []
        errs_hi = []
        for mix in mixes:
            cell = by_mix[mix]
            n = len(cell)
            ok = sum(1 for r in cell if r.get(metric) is True)
            r, lo, hi = wilson_ci(ok, n)
            rates.append(r * 100)
            errs_lo.append((r - lo) * 100)
            errs_hi.append((hi - r) * 100)

        bars = ax.bar(x + i * width, rates, width,
                      yerr=[errs_lo, errs_hi], capsize=4,
                      label=label, color=[COLORS.get(metric, "#999")] * len(mixes),
                      alpha=0.85 if i == 0 else 0.6)

    ax.set_ylabel("Success Rate (%)")
    ax.set_title("Rehydration Quality by Relation Type")
    ax.set_xticks(x + width / 2)
    ax.set_xticklabels([m.capitalize() for m in mixes])
    ax.set_ylim(0, 105)
    ax.legend()
    fig.tight_layout()
    fig.savefig(fig_dir / "01_relation_mix_bars.png")
    plt.close(fig)


def fig_agent_comparison(results, fig_dir):
    """Grouped bar: Task by agent x relation mix."""
    by_agent = defaultdict(list)
    for r in results:
        agent, _ = parse_model(r["model"])
        by_agent[agent].append(r)

    agents = sorted(by_agent.keys())
    mixes = ["explanatory", "structural", "mixed"]

    fig, ax = plt.subplots(figsize=(9, 5))
    x = np.arange(len(agents))
    width = 0.25

    for i, mix in enumerate(mixes):
        rates = []
        for agent in agents:
            cell = [r for r in by_agent[agent] if parse_variant(r["variant"])["mix"] == mix]
            rates.append(rate(cell, "task") * 100)
        ax.bar(x + i * width, rates, width, label=mix.capitalize(),
               color=COLORS.get(mix, "#999"), alpha=0.85)

    ax.set_ylabel("Task Success Rate (%)")
    ax.set_title("Task Identification by Agent and Relation Type")
    ax.set_xticks(x + width)
    ax.set_xticklabels(agents)
    ax.set_ylim(0, 105)
    ax.legend()
    fig.tight_layout()
    fig.savefig(fig_dir / "02_agent_x_mix_task.png")
    plt.close(fig)


def fig_agent_reason(results, fig_dir):
    """Grouped bar: Reason by agent x relation mix."""
    by_agent = defaultdict(list)
    for r in results:
        agent, _ = parse_model(r["model"])
        by_agent[agent].append(r)

    agents = sorted(by_agent.keys())
    mixes = ["explanatory", "structural", "mixed"]

    fig, ax = plt.subplots(figsize=(9, 5))
    x = np.arange(len(agents))
    width = 0.25

    for i, mix in enumerate(mixes):
        rates = []
        for agent in agents:
            cell = [r for r in by_agent[agent] if parse_variant(r["variant"])["mix"] == mix]
            rates.append(rate(cell, "reason") * 100)
        ax.bar(x + i * width, rates, width, label=mix.capitalize(),
               color=COLORS.get(mix, "#999"), alpha=0.85)

    ax.set_ylabel("Reason Preservation Rate (%)")
    ax.set_title("Rationale Preservation by Agent and Relation Type")
    ax.set_xticks(x + width)
    ax.set_xticklabels(agents)
    ax.set_ylim(0, 105)
    ax.legend()
    fig.tight_layout()
    fig.savefig(fig_dir / "03_agent_x_mix_reason.png")
    plt.close(fig)


def fig_scale_effect(results, fig_dir):
    """Line chart: Task rate by scale for each relation mix."""
    scales = ["micro", "meso", "stress"]
    mixes = ["explanatory", "structural", "mixed"]

    fig, ax = plt.subplots(figsize=(8, 5))

    for mix in mixes:
        rates = []
        for scale in scales:
            cell = [r for r in results
                    if parse_variant(r["variant"])["scale"] == scale
                    and parse_variant(r["variant"])["mix"] == mix]
            rates.append(rate(cell, "task") * 100)
        ax.plot(scales, rates, marker="o", linewidth=2, markersize=8,
                label=mix.capitalize(), color=COLORS.get(mix, "#999"))

    ax.set_ylabel("Task Success Rate (%)")
    ax.set_xlabel("Graph Scale")
    ax.set_title("Scale Effect on Task Identification")
    ax.set_ylim(0, 105)
    ax.legend()
    fig.tight_layout()
    fig.savefig(fig_dir / "04_scale_effect.png")
    plt.close(fig)


def fig_prompt_comparison(results, fig_dir):
    """Horizontal bar: Task rate by prompt variant."""
    by_prompt = defaultdict(list)
    for r in results:
        by_prompt[r["prompt"]].append(r)

    prompts = sorted(by_prompt.keys(), key=lambda p: rate(by_prompt[p], "task"), reverse=True)

    fig, ax = plt.subplots(figsize=(9, 4))
    y = np.arange(len(prompts))
    task_rates = [rate(by_prompt[p], "task") * 100 for p in prompts]

    bars = ax.barh(y, task_rates, color="#3498db", alpha=0.85)
    ax.set_yticks(y)
    ax.set_yticklabels(prompts)
    ax.set_xlabel("Task Success Rate (%)")
    ax.set_title("Prompt Variant Effectiveness")
    ax.set_xlim(0, 80)

    for bar, val in zip(bars, task_rates):
        ax.text(bar.get_width() + 1, bar.get_y() + bar.get_height() / 2,
                f"{val:.0f}%", va="center", fontsize=10)

    fig.tight_layout()
    fig.savefig(fig_dir / "05_prompt_comparison.png")
    plt.close(fig)


def fig_judge_bias(results, fig_dir):
    """Grouped bar: same agent scored by different judges."""
    by_agent_judge = defaultdict(list)
    for r in results:
        agent, judge = parse_model(r["model"])
        by_agent_judge[(agent, judge)].append(r)

    agents = sorted(set(a for a, _ in by_agent_judge.keys()))
    judges = sorted(set(j for _, j in by_agent_judge.keys()))

    fig, axes = plt.subplots(1, 3, figsize=(14, 4), sharey=True)

    for idx, metric in enumerate(["task", "restart", "reason"]):
        ax = axes[idx]
        x = np.arange(len(agents))
        width = 0.35

        for i, judge in enumerate(judges):
            rates = []
            for agent in agents:
                cell = by_agent_judge.get((agent, judge), [])
                rates.append(rate(cell, metric) * 100)
            ax.bar(x + i * width, rates, width, label=f"Judge: {judge}",
                   color=COLORS.get(judge, "#999"), alpha=0.85)

        ax.set_ylabel("Success Rate (%)" if idx == 0 else "")
        ax.set_title(metric.capitalize())
        ax.set_xticks(x + width / 2)
        ax.set_xticklabels(agents, fontsize=9)
        ax.set_ylim(0, 105)
        if idx == 0:
            ax.legend(fontsize=8)

    fig.suptitle("Judge Bias: Same Agent, Different Judge", fontsize=13)
    fig.tight_layout()
    fig.savefig(fig_dir / "06_judge_bias.png")
    plt.close(fig)


def fig_noise_impact(results, fig_dir):
    """Grouped bar: clean vs competing noise by relation mix."""
    mixes = ["explanatory", "structural", "mixed"]
    noises = ["clean", "competing"]

    fig, ax = plt.subplots(figsize=(8, 5))
    x = np.arange(len(mixes))
    width = 0.35

    for i, noise in enumerate(noises):
        rates = []
        for mix in mixes:
            cell = [r for r in results
                    if parse_variant(r["variant"])["mix"] == mix
                    and parse_variant(r["variant"])["noise"] == noise]
            rates.append(rate(cell, "task") * 100)
        ax.bar(x + i * width, rates, width, label=noise.capitalize(),
               color=COLORS.get(noise, "#999"), alpha=0.85)

    ax.set_ylabel("Task Success Rate (%)")
    ax.set_title("Noise Impact on Task Identification")
    ax.set_xticks(x + width / 2)
    ax.set_xticklabels([m.capitalize() for m in mixes])
    ax.set_ylim(0, 105)
    ax.legend()
    fig.tight_layout()
    fig.savefig(fig_dir / "07_noise_impact.png")
    plt.close(fig)


def fig_latency_distribution(results, fig_dir):
    """Box plot: latency by agent."""
    by_agent = defaultdict(list)
    for r in results:
        agent, _ = parse_model(r["model"])
        if r.get("latency_ms", 0) > 0:
            by_agent[agent].append(r["latency_ms"])

    agents = sorted(by_agent.keys())
    data = [by_agent[a] for a in agents]

    fig, ax = plt.subplots(figsize=(7, 5))
    bp = ax.boxplot(data, tick_labels=agents, patch_artist=True, showfliers=False)

    for patch, agent in zip(bp["boxes"], agents):
        patch.set_facecolor(COLORS.get(agent, "#999"))
        patch.set_alpha(0.7)

    ax.set_ylabel("Latency (ms)")
    ax.set_title("Inference + Judge Latency by Agent")
    fig.tight_layout()
    fig.savefig(fig_dir / "08_latency_boxplot.png")
    plt.close(fig)


def fig_convergence_heatmap(results, fig_dir):
    """Heatmap: strict vs lenient delta per agent→judge pair."""
    by_cell = defaultdict(list)
    for r in results:
        agent, judge = parse_model(r["model"])
        by_cell[(agent, judge, r["prompt"])].append(r)

    pairs = sorted(set((a, j) for (a, j, _) in by_cell.keys()))
    metrics = ["task", "restart", "reason"]

    data = []
    row_labels = []
    for agent, judge in pairs:
        for metric in metrics:
            strict = by_cell.get((agent, judge, "strict-judge"), [])
            lenient = by_cell.get((agent, judge, "lenient-judge"), [])
            s_ok = sum(1 for r in strict if r.get(metric) is True)
            l_ok = sum(1 for r in lenient if r.get(metric) is True)
            data.append(abs(s_ok - l_ok))
            row_labels.append(f"{agent}\u2192{judge} {metric}")

    n_rows = len(pairs) * 3
    matrix = np.array(data).reshape(n_rows, 1)

    fig, ax = plt.subplots(figsize=(4, 8))
    im = ax.imshow(matrix, cmap="RdYlGn_r", aspect="auto", vmin=0, vmax=10)

    ax.set_yticks(range(n_rows))
    ax.set_yticklabels(row_labels, fontsize=8)
    ax.set_xticks([0])
    ax.set_xticklabels(["|Strict - Lenient|"])
    ax.set_title("Judge Prompt Convergence\n(lower = better)")

    for i in range(n_rows):
        ax.text(0, i, str(int(matrix[i, 0])), ha="center", va="center", fontsize=10,
                color="white" if matrix[i, 0] > 5 else "black")

    fig.colorbar(im, ax=ax, shrink=0.5)
    fig.tight_layout()
    fig.savefig(fig_dir / "09_convergence_heatmap.png")
    plt.close(fig)


def fig_judge_strictness(results, fig_dir):
    """Radar/bar: judge acceptance rate per metric — measures judge rigor.

    A lenient judge accepts more, a strict judge less. Comparing two judges
    on the same agent responses reveals judge bias vs actual quality.
    """
    by_judge = defaultdict(list)
    for r in results:
        _, judge = parse_model(r["model"])
        by_judge[judge].append(r)

    judges = sorted(by_judge.keys())
    metrics = ["task", "restart", "reason"]
    metric_labels = ["Task", "Restart", "Reason"]

    fig, axes = plt.subplots(1, 2, figsize=(12, 5))

    # Left: acceptance rate per judge per metric
    ax = axes[0]
    x = np.arange(len(metrics))
    width = 0.35

    for i, judge in enumerate(judges):
        cell = by_judge[judge]
        rates = [rate(cell, m) * 100 for m in metrics]
        ax.bar(x + i * width, rates, width, label=f"Judge: {judge}",
               color=COLORS.get(judge, "#999"), alpha=0.85)

    ax.set_ylabel("Acceptance Rate (%)")
    ax.set_title("Judge Strictness: Acceptance Rate by Metric")
    ax.set_xticks(x + width / 2)
    ax.set_xticklabels(metric_labels)
    ax.set_ylim(0, 105)
    ax.legend()

    # Right: per-agent delta between judges (measures bias)
    ax2 = axes[1]
    agents = sorted(set(parse_model(r["model"])[0] for r in results))

    x2 = np.arange(len(agents))
    width2 = 0.25

    for i, metric in enumerate(metrics):
        deltas = []
        for agent in agents:
            rates_by_judge = {}
            for judge in judges:
                cell = [r for r in results
                        if parse_model(r["model"]) == (agent, judge)]
                rates_by_judge[judge] = rate(cell, metric) * 100

            vals = list(rates_by_judge.values())
            delta = max(vals) - min(vals) if len(vals) >= 2 else 0
            deltas.append(delta)

        ax2.bar(x2 + i * width2, deltas, width2, label=metric_labels[i],
                alpha=0.85)

    ax2.set_ylabel("Judge Delta (pp)")
    ax2.set_title("Judge Disagreement by Agent\n(lower = more consistent)")
    ax2.set_xticks(x2 + width2)
    ax2.set_xticklabels(agents, fontsize=9)
    ax2.set_ylim(0, 60)
    ax2.legend(fontsize=9)

    fig.tight_layout()
    fig.savefig(fig_dir / "10_judge_strictness.png")
    plt.close(fig)


def parse_captures_from_log(run_dir):
    """Parse [CAPTURE] lines from test.log to get kernel metrics per variant."""
    log_path = run_dir / "test.log"
    captures = {}
    if not log_path.exists():
        return captures
    with open(log_path) as f:
        for line in f:
            if "[CAPTURE]" in line:
                # [CAPTURE] micro-ops-explanatory-clean: 370 tokens, reason=...
                after = line.split("[CAPTURE]", 1)[1].strip()
                parts = after.split(":", 1)
                variant = parts[0].strip()
                rest = parts[1].strip() if len(parts) > 1 else ""
                tokens = 0
                has_reason = False
                if "tokens" in rest:
                    tok_str = rest.split("tokens")[0].strip()
                    try:
                        tokens = int(tok_str)
                    except ValueError:
                        pass
                if "reason=" in rest:
                    reason = rest.split("reason=", 1)[1].strip()
                    has_reason = reason != "none"
                captures[variant] = {"tokens": tokens, "has_reason": has_reason}
    return captures


def fig_kernel_token_efficiency(results, captures, fig_dir):
    """Kernel metric: tokens rendered per variant, grouped by mix and scale.

    Measures how efficiently the kernel compresses the graph into context.
    """
    mixes = ["explanatory", "structural", "mixed"]
    scales = ["micro", "meso", "stress"]

    fig, axes = plt.subplots(1, 2, figsize=(13, 5))

    # Left: tokens by mix (averaged across scales)
    ax = axes[0]
    for mix in mixes:
        tokens_by_scale = []
        for scale in scales:
            variant_tokens = [
                c["tokens"] for v, c in captures.items()
                if f"-{mix}-" in v and v.startswith(scale)
            ]
            if variant_tokens:
                tokens_by_scale.append(np.mean(variant_tokens))
        ax.plot(scales, tokens_by_scale, marker="o", linewidth=2, markersize=8,
                label=mix.capitalize(), color=COLORS.get(mix, "#999"))

    ax.set_ylabel("Rendered Tokens")
    ax.set_xlabel("Graph Scale")
    ax.set_title("Kernel Token Output by Scale and Relation Type")
    ax.legend()

    # Right: information density (Task success per 100 tokens)
    ax2 = axes[1]
    x = np.arange(len(mixes))
    width = 0.25

    for i, scale in enumerate(scales):
        densities = []
        for mix in mixes:
            # Get average tokens for this mix+scale
            variant_tokens = [
                c["tokens"] for v, c in captures.items()
                if f"-{mix}-" in v and v.startswith(scale)
            ]
            avg_tokens = np.mean(variant_tokens) if variant_tokens else 1

            # Get task success rate for this mix+scale
            cell = [r for r in results
                    if parse_variant(r["variant"])["mix"] == mix
                    and parse_variant(r["variant"])["scale"] == scale]
            task_rate = rate(cell, "task")

            # Information density: task success per 100 tokens
            density = (task_rate * 100) / (avg_tokens / 100)
            densities.append(density)

        ax2.bar(x + i * width, densities, width, label=scale.capitalize(),
                color=COLORS.get(scale, "#999"), alpha=0.85)

    ax2.set_ylabel("Task Success per 100 Tokens")
    ax2.set_xlabel("Relation Mix")
    ax2.set_title("Information Density: Task Success / Token Budget")
    ax2.set_xticks(x + width)
    ax2.set_xticklabels([m.capitalize() for m in mixes])
    ax2.legend()

    fig.tight_layout()
    fig.savefig(fig_dir / "11_kernel_token_efficiency.png")
    plt.close(fig)


def fig_kernel_causal_signal(captures, fig_dir):
    """Kernel metric: ratio of variants with causal rationale vs without.

    Directly measures whether the kernel preserves causal signal in rendering.
    """
    by_mix = defaultdict(lambda: {"with_reason": 0, "without_reason": 0, "total_tokens": 0})

    for variant, data in captures.items():
        v = parse_variant(variant)
        mix = v["mix"]
        if data["has_reason"]:
            by_mix[mix]["with_reason"] += 1
        else:
            by_mix[mix]["without_reason"] += 1
        by_mix[mix]["total_tokens"] += data["tokens"]

    mixes = ["explanatory", "structural", "mixed"]

    fig, axes = plt.subplots(1, 2, figsize=(12, 5))

    # Left: causal signal presence
    ax = axes[0]
    with_r = [by_mix[m]["with_reason"] for m in mixes]
    without_r = [by_mix[m]["without_reason"] for m in mixes]

    x = np.arange(len(mixes))
    ax.bar(x, with_r, 0.6, label="Has causal rationale", color="#2ecc71", alpha=0.85)
    ax.bar(x, without_r, 0.6, bottom=with_r, label="No rationale", color="#e74c3c", alpha=0.85)

    ax.set_ylabel("Variant Count")
    ax.set_title("Causal Signal in Rendered Context")
    ax.set_xticks(x)
    ax.set_xticklabels([m.capitalize() for m in mixes])
    ax.legend()

    # Right: average tokens by mix
    ax2 = axes[1]
    avg_tokens = []
    for mix in mixes:
        total_variants = by_mix[mix]["with_reason"] + by_mix[mix]["without_reason"]
        avg = by_mix[mix]["total_tokens"] / total_variants if total_variants > 0 else 0
        avg_tokens.append(avg)

    bars = ax2.bar(x, avg_tokens, 0.6,
                   color=[COLORS.get(m, "#999") for m in mixes], alpha=0.85)
    ax2.set_ylabel("Average Tokens")
    ax2.set_title("Token Cost by Relation Type")
    ax2.set_xticks(x)
    ax2.set_xticklabels([m.capitalize() for m in mixes])

    for bar, val in zip(bars, avg_tokens):
        ax2.text(bar.get_x() + bar.get_width() / 2, bar.get_height() + 10,
                 f"{val:.0f}", ha="center", fontsize=10)

    fig.tight_layout()
    fig.savefig(fig_dir / "12_kernel_causal_signal.png")
    plt.close(fig)


def fig_hypothesis_summary(results, captures, fig_dir):
    """Summary figure: the five hypotheses this benchmark tests."""
    fig, ax = plt.subplots(figsize=(12, 7))
    ax.axis("off")

    hypotheses = [
        ("H1: Rehydration adds value",
         "Explanatory > Structural",
         rate([r for r in results if parse_variant(r["variant"])["mix"] == "explanatory"], "task"),
         rate([r for r in results if parse_variant(r["variant"])["mix"] == "structural"], "task")),
        ("H2: Small models benefit",
         "Qwen3-8B expl. Task rate",
         rate([r for r in results
               if parse_model(r["model"])[0] == "qwen3-8b"
               and parse_variant(r["variant"])["mix"] == "explanatory"], "task"),
         rate([r for r in results
               if parse_model(r["model"])[0] == "qwen3-8b"
               and parse_variant(r["variant"])["mix"] == "structural"], "task")),
        ("H3: Judge prompt is reliable",
         "Strict-Lenient delta (avg)",
         None, None),
        ("H4: Causal metadata is the signal",
         "Reason preservation: expl vs struct",
         rate([r for r in results if parse_variant(r["variant"])["mix"] == "explanatory"], "reason"),
         rate([r for r in results if parse_variant(r["variant"])["mix"] == "structural"], "reason")),
        ("H5: Test infra is consistent",
         "Clean vs Competing noise",
         rate([r for r in results if parse_variant(r["variant"])["noise"] == "clean"], "task"),
         rate([r for r in results if parse_variant(r["variant"])["noise"] == "competing"], "task")),
    ]

    y_pos = 0.92
    ax.text(0.5, 0.98, "Hypothesis Validation Summary", fontsize=16,
            fontweight="bold", ha="center", va="top", transform=ax.transAxes)

    for hyp_name, measure, val_a, val_b in hypotheses:
        if val_a is not None and val_b is not None:
            verdict = "SUPPORTED" if val_a > val_b + 0.05 else ("MIXED" if abs(val_a - val_b) <= 0.05 else "NOT SUPPORTED")
            color = "#2ecc71" if verdict == "SUPPORTED" else ("#f39c12" if verdict == "MIXED" else "#e74c3c")
            detail = f"{val_a:.0%} vs {val_b:.0%}"
        else:
            # H3: compute avg strict-lenient delta
            by_cell = defaultdict(list)
            for r in results:
                agent, judge = parse_model(r["model"])
                by_cell[(agent, judge, r["prompt"])].append(r)

            deltas = []
            for (agent, judge) in set((a, j) for (a, j, _) in by_cell.keys()):
                for metric in ["task", "restart", "reason"]:
                    strict = by_cell.get((agent, judge, "strict-judge"), [])
                    lenient = by_cell.get((agent, judge, "lenient-judge"), [])
                    s = sum(1 for r in strict if r.get(metric) is True)
                    l = sum(1 for r in lenient if r.get(metric) is True)
                    deltas.append(abs(s - l))
            avg_delta = np.mean(deltas) if deltas else 0
            verdict = "SUPPORTED" if avg_delta < 5 else "MIXED"
            color = "#2ecc71" if verdict == "SUPPORTED" else "#f39c12"
            detail = f"avg delta = {avg_delta:.1f} evals"

        ax.text(0.05, y_pos, f"\u25cf {hyp_name}", fontsize=12,
                fontweight="bold", va="top", transform=ax.transAxes)
        ax.text(0.55, y_pos, measure, fontsize=11, va="top",
                transform=ax.transAxes, color="#555")
        ax.text(0.55, y_pos - 0.04, detail, fontsize=11, va="top",
                transform=ax.transAxes)
        ax.text(0.92, y_pos, verdict, fontsize=12, fontweight="bold",
                va="top", transform=ax.transAxes, color=color,
                bbox=dict(boxstyle="round,pad=0.3", facecolor=color, alpha=0.15))
        y_pos -= 0.15

    fig.tight_layout()
    fig.savefig(fig_dir / "00_hypothesis_summary.png")
    plt.close(fig)


def main():
    if len(sys.argv) > 1:
        run_dir = Path(sys.argv[1])
    else:
        run_dir = find_latest_run()

    fig_dir = run_dir / "figures"
    fig_dir.mkdir(exist_ok=True)

    print(f"Generating figures from: {run_dir}", file=sys.stderr)
    results = load_results(run_dir)
    print(f"Loaded {len(results)} evaluations", file=sys.stderr)

    setup_style()

    fig_relation_mix_bars(results, fig_dir)
    print("  01_relation_mix_bars.png", file=sys.stderr)

    fig_agent_comparison(results, fig_dir)
    print("  02_agent_x_mix_task.png", file=sys.stderr)

    fig_agent_reason(results, fig_dir)
    print("  03_agent_x_mix_reason.png", file=sys.stderr)

    fig_scale_effect(results, fig_dir)
    print("  04_scale_effect.png", file=sys.stderr)

    fig_prompt_comparison(results, fig_dir)
    print("  05_prompt_comparison.png", file=sys.stderr)

    fig_judge_bias(results, fig_dir)
    print("  06_judge_bias.png", file=sys.stderr)

    fig_noise_impact(results, fig_dir)
    print("  07_noise_impact.png", file=sys.stderr)

    fig_latency_distribution(results, fig_dir)
    print("  08_latency_boxplot.png", file=sys.stderr)

    fig_convergence_heatmap(results, fig_dir)
    print("  09_convergence_heatmap.png", file=sys.stderr)

    fig_judge_strictness(results, fig_dir)
    print("  10_judge_strictness.png", file=sys.stderr)

    captures = parse_captures_from_log(run_dir)
    if captures:
        fig_kernel_token_efficiency(results, captures, fig_dir)
        print("  11_kernel_token_efficiency.png", file=sys.stderr)

        fig_kernel_causal_signal(captures, fig_dir)
        print("  12_kernel_causal_signal.png", file=sys.stderr)

        fig_hypothesis_summary(results, captures, fig_dir)
        print("  00_hypothesis_summary.png", file=sys.stderr)
    else:
        print("  (skipped kernel figures — no [CAPTURE] data in test.log)", file=sys.stderr)

    n_figs = len(list(fig_dir.glob("*.png")))
    print(f"\n{n_figs} figures written to: {fig_dir}", file=sys.stderr)


if __name__ == "__main__":
    main()
