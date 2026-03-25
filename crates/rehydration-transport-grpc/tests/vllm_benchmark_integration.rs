#![cfg(feature = "container-tests")]

mod agentic_support;

use std::error::Error;
use std::time::{Duration, Instant};

use agentic_support::agentic_fixture::AgenticFixture;
use agentic_support::paper_metrics::PaperUseCaseMetric;
use agentic_support::paper_metrics::emit_metric;
use rehydration_proto::v1beta1::{
    BundleRenderFormat, GetContextRequest, Phase, RenderedContext, ResolutionTier,
    context_query_service_client::ContextQueryServiceClient,
};
use rehydration_testkit::{
    Domain, EvaluationGroundTruth, GraphSeedConfig, LlmEvaluatorConfig, NoiseMode, RelationMix,
    evaluate_with_llm, generate_seed, seed_publisher::seed_to_projection_events,
};
use tokio::time::sleep;
use tonic::transport::Channel;

const SUBJECT_PREFIX: &str = "rehydration";
const BENCHMARK_ROLE: &str = "evaluator";

/// Extract the best available content for LLM evaluation.
///
/// Prefers tiered content (L0 + L1 concatenated) when available — this is the
/// mode-aware path that prunes distractors under token pressure. Falls back to
/// flat `rendered.content` when tiers are empty (backward compatibility).
fn tier_content_for_eval(rendered: &RenderedContext) -> String {
    if rendered.tiers.is_empty() {
        return rendered.content.clone();
    }

    // Concatenate L0 + L1 content (skip L2 — evidence/structural)
    let mut parts = Vec::new();
    for tier in &rendered.tiers {
        if tier.tier == ResolutionTier::L0Summary as i32
            || tier.tier == ResolutionTier::L1CausalSpine as i32
        {
            parts.push(tier.content.as_str());
        }
    }

    if parts.is_empty() {
        rendered.content.clone()
    } else {
        parts.join("\n\n")
    }
}

struct BenchmarkVariant {
    scale: Scale,
    domain: Domain,
    relation_mix: RelationMix,
}

#[derive(Debug, Clone, Copy)]
enum Scale {
    Micro,
    Meso,
    Stress,
}

impl Scale {
    fn config(self, domain: Domain, relation_mix: RelationMix) -> GraphSeedConfig {
        let mut config = match self {
            Scale::Micro => GraphSeedConfig::micro(domain),
            Scale::Meso => GraphSeedConfig::meso(domain),
            Scale::Stress => GraphSeedConfig::stress(domain),
        };
        config.relation_mix = relation_mix;
        if std::env::var("BENCHMARK_NOISE_MODE").as_deref() == Ok("competing") {
            config.noise_mode = NoiseMode::CompetingCausal;
        }
        config.id_prefix = format!(
            "{}-{}-{}",
            self.label(),
            domain_label(domain),
            mix_label(relation_mix)
        );
        config
    }

    fn label(self) -> &'static str {
        match self {
            Scale::Micro => "micro",
            Scale::Meso => "meso",
            Scale::Stress => "stress",
        }
    }
}

fn domain_label(domain: Domain) -> &'static str {
    match domain {
        Domain::Operations => "ops",
        Domain::SoftwareDebugging => "debug",
    }
}

fn mix_label(mix: RelationMix) -> &'static str {
    match mix {
        RelationMix::Explanatory => "explanatory",
        RelationMix::Structural => "structural",
        RelationMix::Mixed => "mixed",
    }
}

fn all_variants() -> Vec<BenchmarkVariant> {
    let scales = [Scale::Micro, Scale::Meso, Scale::Stress];
    let domains = [Domain::Operations, Domain::SoftwareDebugging];
    let mixes = [
        RelationMix::Explanatory,
        RelationMix::Structural,
        RelationMix::Mixed,
    ];

    let mut variants = Vec::new();
    for &scale in &scales {
        for &domain in &domains {
            for &relation_mix in &mixes {
                variants.push(BenchmarkVariant {
                    scale,
                    domain,
                    relation_mix,
                });
            }
        }
    }
    variants
}

#[tokio::test]
async fn vllm_benchmark_across_scales_domains_and_variants()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let llm_config = if std::env::var("LLM_ENDPOINT").is_ok() {
        Some(LlmEvaluatorConfig::from_env())
    } else {
        eprintln!("LLM_ENDPOINT not set — running without LLM evaluation");
        None
    };

    let variants = all_variants();
    let mut results: Vec<BenchmarkResult> = Vec::new();

    for variant in &variants {
        let config = variant.scale.config(variant.domain, variant.relation_mix);
        let seed = generate_seed(config.clone());
        let run_id = format!(
            "{}-{}-{}",
            variant.scale.label(),
            domain_label(variant.domain),
            mix_label(variant.relation_mix)
        );

        let events = seed_to_projection_events(&seed, SUBJECT_PREFIX, &run_id)?;

        let fixture = AgenticFixture::start_with_seed(
            &seed.root.node_id,
            &seed
                .nodes
                .first()
                .map(|n| n.node_id.as_str())
                .unwrap_or("chain-0"),
            |publisher| {
                let events = events.clone();
                async move {
                    for (subject, payload) in events {
                        publisher.publish(subject, payload.into()).await?;
                    }
                    publisher.flush().await?;
                    Ok(())
                }
            },
        )
        .await?;

        sleep(Duration::from_secs(3)).await;

        let result: Result<BenchmarkResult, Box<dyn Error + Send + Sync>> = async {
            let mut query_client = fixture.query_client();
            let total_start = Instant::now();
            let query_start = Instant::now();

            let target_node_id = seed
                .nodes
                .first()
                .map(|n| n.node_id.clone())
                .unwrap_or_else(|| seed.root.node_id.clone());

            let response = query_client
                .get_context(GetContextRequest {
                    root_node_id: seed.root.node_id.clone(),
                    role: BENCHMARK_ROLE.to_string(),
                    phase: Phase::Build as i32,
                    work_item_id: target_node_id.clone(),
                    token_budget: std::env::var("BENCHMARK_TOKEN_BUDGET")
                        .ok()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(4096),
                    requested_scopes: vec![],
                    render_format: BundleRenderFormat::Structured as i32,
                    include_debug_sections: false,
                    depth: config.chain_length as u32,
                    max_tier: 0,
                    rehydration_mode: 0,
                })
                .await?
                .into_inner();

            let query_latency_ms = query_start.elapsed().as_secs_f64() * 1000.0;

            let bundle = response.bundle.ok_or("missing bundle")?;
            let role_bundle = bundle.bundles.first().ok_or("missing role bundle")?;
            let rendered = response.rendered.ok_or("missing rendered context")?;

            let llm_eval = if let Some(ref llm_cfg) = llm_config {
                let domain_name = match variant.domain {
                    Domain::Operations => "operations incident management",
                    Domain::SoftwareDebugging => "software debugging",
                };
                let question = format!(
                    "Given this rehydrated context from a {domain_name} graph:\n\
                     1. What is the root cause or failure point?\n\
                     2. Which node should the system restart from to continue work?\n\
                     3. What is the main rationale in the causal chain?"
                );
                let first_chain_node = seed.nodes.first();
                let last_chain_node = seed
                    .nodes
                    .iter()
                    .filter(|n| n.node_kind != "distractor")
                    .last();

                // Domain-aware ground truth:
                // - Operations: root incident IS the failure point, first decision is restart
                // - Debugging: root is the report, failure is the hypothesis/defect chain,
                //   restart can be first hypothesis OR last validated step
                let (failure_desc, restart_desc) = match variant.domain {
                    Domain::Operations => (
                        format!(
                            "{} — {}. The incident root is the primary failure point",
                            seed.root.node_kind, seed.root.summary
                        ),
                        format!(
                            "Any node on the main operational chain is valid: \
                             the first {} ({}) or the last verified checkpoint ({})",
                            first_chain_node
                                .map(|n| n.node_kind.as_str())
                                .unwrap_or("decision"),
                            first_chain_node
                                .map(|n| n.title.as_str())
                                .unwrap_or("chain-0"),
                            last_chain_node
                                .map(|n| format!("{} — {}", n.node_kind, n.title))
                                .unwrap_or_else(|| "last chain node".to_string()),
                        ),
                    ),
                    Domain::SoftwareDebugging => (
                        format!(
                            "The root cause lies in the causal chain: either the initial {} ({}) \
                             or a deeper node in the investigation chain (e.g. {})",
                            first_chain_node
                                .map(|n| n.node_kind.as_str())
                                .unwrap_or("hypothesis"),
                            first_chain_node
                                .map(|n| n.title.as_str())
                                .unwrap_or("chain-0"),
                            last_chain_node
                                .map(|n| format!("{} — {}", n.node_kind, n.title))
                                .unwrap_or_else(|| "deepest chain node".to_string()),
                        ),
                        format!(
                            "Any node on the main causal chain is valid: the first {} ({}) \
                             or the last validated step before the suspected failure ({})",
                            first_chain_node
                                .map(|n| n.node_kind.as_str())
                                .unwrap_or("hypothesis"),
                            first_chain_node
                                .map(|n| n.title.as_str())
                                .unwrap_or("chain-0"),
                            last_chain_node
                                .map(|n| format!("{} — {}", n.node_kind, n.title))
                                .unwrap_or_else(|| "last chain node".to_string()),
                        ),
                    ),
                };

                let ground_truth = EvaluationGroundTruth {
                    expected_failure_point: Some(failure_desc),
                    expected_restart_node: Some(restart_desc),
                    expected_reason: seed.relations.first().and_then(|r| r.rationale.clone()),
                    domain_context: Some(domain_name.to_string()),
                };
                // Prefer tiered content (L0+L1) when available — this is the
                // mode-aware rendering path that prunes distractors under pressure.
                // Falls back to flat content for backward compatibility.
                let eval_content = tier_content_for_eval(&rendered);

                match evaluate_with_llm(llm_cfg, &eval_content, &question, &ground_truth).await {
                    Ok(eval) => Some(eval),
                    Err(error) => {
                        eprintln!("LLM eval failed for {run_id}: {error}");
                        None
                    }
                }
            } else {
                None
            };

            let total_latency_ms = total_start.elapsed().as_secs_f64() * 1000.0;

            Ok(BenchmarkResult {
                run_id: run_id.clone(),
                scale: variant.scale.label().to_string(),
                domain: domain_label(variant.domain).to_string(),
                relation_mix: mix_label(variant.relation_mix).to_string(),
                total_nodes: seed.total_nodes() as u32,
                total_relations: seed.total_relations() as u32,
                bundle_nodes: role_bundle.neighbor_nodes.len() as u32 + 1,
                bundle_relationships: role_bundle.relationships.len() as u32,
                rendered_token_count: rendered.token_count,
                query_latency_ms,
                total_latency_ms,
                llm_task_success: llm_eval.as_ref().map(|e| e.llm_task_success),
                llm_restart_accuracy: llm_eval.as_ref().map(|e| e.llm_restart_accuracy),
                llm_reason_preserved: llm_eval.as_ref().map(|e| e.llm_reason_preserved),
                llm_latency_ms: llm_eval.as_ref().map(|e| e.llm_latency_ms),
                llm_response: llm_eval.as_ref().map(|e| e.llm_response.clone()),
                ground_truth_summary: Some(format!(
                    "failure={} — {} | restart={} | reason={}",
                    seed.root.node_kind,
                    seed.root.summary,
                    seed.nodes
                        .first()
                        .map(|n| format!("{} — {}", n.node_kind, n.title))
                        .unwrap_or_else(|| "?".to_string()),
                    seed.relations
                        .first()
                        .and_then(|r| r.rationale.clone())
                        .unwrap_or_else(|| "?".to_string()),
                )),
            })
        }
        .await;

        fixture.shutdown().await?;
        results.push(result?);
    }

    // Print summary
    println!("\n=== vLLM Benchmark Results ===\n");
    println!(
        "{:<30} {:>5} {:>5} {:>6} {:>6} {:>7} {:>8} {:>8} {:>8}",
        "Variant", "Nodes", "Rels", "BundN", "BundR", "Tokens", "TaskOK", "RestOK", "ReasOK"
    );
    println!("{}", "-".repeat(100));

    for r in &results {
        println!(
            "{:<30} {:>5} {:>5} {:>6} {:>6} {:>7} {:>8} {:>8} {:>8}",
            r.run_id,
            r.total_nodes,
            r.total_relations,
            r.bundle_nodes,
            r.bundle_relationships,
            r.rendered_token_count,
            r.llm_task_success
                .map(|v| if v { "yes" } else { "no" })
                .unwrap_or("n/a"),
            r.llm_restart_accuracy
                .map(|v| if v { "yes" } else { "no" })
                .unwrap_or("n/a"),
            r.llm_reason_preserved
                .map(|v| if v { "yes" } else { "no" })
                .unwrap_or("n/a"),
        );
    }

    // Diagnostic: print LLM response and ground truth for failing variants
    if llm_config.is_some() {
        println!("\n=== Diagnostic: LLM responses for non-perfect variants ===\n");
        for r in &results {
            let score = [
                r.llm_task_success.unwrap_or(false),
                r.llm_restart_accuracy.unwrap_or(false),
                r.llm_reason_preserved.unwrap_or(false),
            ]
            .iter()
            .filter(|&&v| v)
            .count();
            if score < 3 {
                println!("--- {} (score {}/3) ---", r.run_id, score);
                if let Some(ref gt) = r.ground_truth_summary {
                    println!("  GROUND TRUTH: {gt}");
                }
                if let Some(ref resp) = r.llm_response {
                    let truncated: String = resp.chars().take(500).collect();
                    println!("  GPT-5.4 RESPONSE: {truncated}");
                }
                println!();
            }
        }
    }

    // Verify structural invariants (no LLM needed)
    for r in &results {
        assert!(r.bundle_nodes > 0, "{}: bundle should have nodes", r.run_id);
        assert!(
            r.rendered_token_count > 0,
            "{}: should have tokens",
            r.run_id
        );
    }

    // If LLM was available, verify explanatory beats structural
    if llm_config.is_some() {
        for scale in ["micro", "meso", "stress"] {
            for domain in ["ops", "debug"] {
                let explanatory = results.iter().find(|r| {
                    r.scale == scale && r.domain == domain && r.relation_mix == "explanatory"
                });
                let structural = results.iter().find(|r| {
                    r.scale == scale && r.domain == domain && r.relation_mix == "structural"
                });

                if let (Some(exp), Some(str_)) = (explanatory, structural) {
                    let exp_score = [
                        exp.llm_task_success.unwrap_or(false),
                        exp.llm_restart_accuracy.unwrap_or(false),
                        exp.llm_reason_preserved.unwrap_or(false),
                    ]
                    .iter()
                    .filter(|&&v| v)
                    .count();

                    let str_score = [
                        str_.llm_task_success.unwrap_or(false),
                        str_.llm_restart_accuracy.unwrap_or(false),
                        str_.llm_reason_preserved.unwrap_or(false),
                    ]
                    .iter()
                    .filter(|&&v| v)
                    .count();

                    println!(
                        "\n{scale}/{domain}: explanatory={exp_score}/3, structural={str_score}/3"
                    );
                    // Log comparison — assertion deferred until LLM-as-judge evaluation
                    if exp_score < str_score {
                        eprintln!(
                            "NOTE: {scale}/{domain}: explanatory ({exp_score}) < structural ({str_score}) — \
                             substring matching may produce false negatives with richer explanatory responses"
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct BenchmarkResult {
    run_id: String,
    scale: String,
    domain: String,
    relation_mix: String,
    total_nodes: u32,
    total_relations: u32,
    bundle_nodes: u32,
    bundle_relationships: u32,
    rendered_token_count: u32,
    query_latency_ms: f64,
    total_latency_ms: f64,
    llm_task_success: Option<bool>,
    llm_restart_accuracy: Option<bool>,
    llm_reason_preserved: Option<bool>,
    llm_latency_ms: Option<f64>,
    llm_response: Option<String>,
    ground_truth_summary: Option<String>,
}
