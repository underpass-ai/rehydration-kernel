#![cfg(feature = "container-tests")]

//! Integration tests for multi-resolution tiers (L0/L1/L2).
//!
//! These tests verify that tiers are populated in real gRPC responses
//! and that `max_tier` filtering works end-to-end through the full stack.

mod agentic_support;

use std::error::Error;

use agentic_support::agentic_fixture::AgenticFixture;
use agentic_support::explanatory_seed_data::{
    DetailMode, FAILURE_FOCUS_NODE_ID, ProjectionSeedVariant, ROOT_NODE_ID,
    publish_flawed_task_projection_events_variant,
};
use rehydration_proto::v1beta1::{
    BundleRenderFormat, GetContextRequest, Phase, ResolutionTier,
    context_query_service_client::ContextQueryServiceClient,
};
use tonic::transport::Channel;

const TOKEN_BUDGET: u32 = 4096;

async fn start_explanatory_fixture() -> Result<AgenticFixture, Box<dyn Error + Send + Sync>> {
    let variant = ProjectionSeedVariant::FULL_EXPLANATORY_WITH_DETAIL;
    AgenticFixture::start_with_seed_and_readiness(
        ROOT_NODE_ID,
        FAILURE_FOCUS_NODE_ID,
        variant.detail_mode == DetailMode::WithDetail,
        |publisher| async move {
            publish_flawed_task_projection_events_variant(&publisher, variant).await
        },
    )
    .await
}

async fn get_context_with_max_tier(
    client: &mut ContextQueryServiceClient<Channel>,
    max_tier: i32,
) -> Result<rehydration_proto::v1beta1::GetContextResponse, Box<dyn Error + Send + Sync>> {
    Ok(client
        .get_context(GetContextRequest {
            root_node_id: ROOT_NODE_ID.to_string(),
            role: "implementer".to_string(),
            phase: Phase::Build as i32,
            work_item_id: FAILURE_FOCUS_NODE_ID.to_string(),
            token_budget: TOKEN_BUDGET,
            requested_scopes: vec![],
            render_format: BundleRenderFormat::Structured as i32,
            include_debug_sections: false,
            depth: 3,
            max_tier,
            rehydration_mode: 0,
        })
        .await?
        .into_inner())
}

#[tokio::test]
async fn tiers_are_populated_in_grpc_response_with_explanatory_data()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let fixture = start_explanatory_fixture().await?;
    let mut client = fixture.query_client();

    let response = get_context_with_max_tier(&mut client, 0).await?;
    let rendered = response.rendered.expect("rendered should exist");

    // Flat content still works
    assert!(!rendered.content.is_empty());
    assert!(rendered.token_count > 0);
    assert!(!rendered.sections.is_empty());

    // Tiers are populated
    assert!(
        rendered.tiers.len() >= 2,
        "should have at least L0 + L1, got {} tiers",
        rendered.tiers.len()
    );

    // L0 Summary
    let l0 = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L0Summary as i32)
        .expect("L0 tier should exist");
    assert!(
        l0.content.contains("Objective:"),
        "L0 should have Objective line"
    );
    assert!(l0.content.contains("Status:"), "L0 should have Status line");
    assert!(
        l0.content.contains("Blocker:"),
        "L0 should have Blocker line"
    );
    assert!(l0.content.contains("Next:"), "L0 should have Next line");
    assert!(l0.token_count > 0);
    assert!(
        l0.token_count <= 150,
        "L0 should fit in ~100 tokens, got {}",
        l0.token_count
    );

    // L1 Causal Spine — should have causal relationships
    let l1 = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L1CausalSpine as i32)
        .expect("L1 tier should exist");
    assert!(
        l1.content.contains("[causal]")
            || l1.content.contains("[motivational]")
            || l1.content.contains("[evidential]"),
        "L1 should contain explanatory relationships"
    );
    assert!(
        !l1.content.contains("Detail "),
        "L1 should not contain node details"
    );
    assert!(l1.token_count > 0);

    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn l0_summary_is_self_contained_for_status_check() -> Result<(), Box<dyn Error + Send + Sync>>
{
    let fixture = start_explanatory_fixture().await?;
    let mut client = fixture.query_client();

    let response = get_context_with_max_tier(&mut client, 0).await?;
    let rendered = response.rendered.expect("rendered should exist");

    let l0 = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L0Summary as i32)
        .expect("L0 should exist");

    // L0 should have exactly 4 lines
    let lines: Vec<_> = l0.content.lines().collect();
    assert_eq!(
        lines.len(),
        4,
        "L0 should be 4 lines: Objective, Status, Blocker, Next. Got:\n{}",
        l0.content
    );
    assert!(lines[0].starts_with("Objective:"));
    assert!(lines[1].starts_with("Status:"));
    assert!(lines[2].starts_with("Blocker:"));
    assert!(lines[3].starts_with("Next:"));

    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn l1_causal_spine_contains_explanatory_but_not_structural()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let fixture = start_explanatory_fixture().await?;
    let mut client = fixture.query_client();

    let response = get_context_with_max_tier(&mut client, 0).await?;
    let rendered = response.rendered.expect("rendered should exist");

    let l1 = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L1CausalSpine as i32)
        .expect("L1 should exist");

    // L1 should have the root node
    assert!(
        l1.content.contains("Node "),
        "L1 should contain the root node render"
    );

    // L1 should have explanatory relationships but NOT structural
    let has_explanatory = l1.content.contains("[causal]")
        || l1.content.contains("[motivational]")
        || l1.content.contains("[evidential]")
        || l1.content.contains("[constraint]");
    assert!(has_explanatory, "L1 should have explanatory relationships");

    let has_structural = l1.content.contains("[structural]");
    assert!(
        !has_structural,
        "L1 should NOT have structural relationships"
    );

    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn l2_evidence_pack_contains_details_and_structural()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let fixture = start_explanatory_fixture().await?;
    let mut client = fixture.query_client();

    let response = get_context_with_max_tier(&mut client, 0).await?;
    let rendered = response.rendered.expect("rendered should exist");

    let l2 = rendered
        .tiers
        .iter()
        .find(|t| t.tier == ResolutionTier::L2EvidencePack as i32);

    // L2 might be empty if there are no structural relationships or details
    // beyond what L1 covers, but if present it should have details
    if let Some(l2) = l2 {
        let has_detail = l2.content.contains("Detail ");
        let has_structural =
            l2.content.contains("[structural]") || l2.content.contains("[procedural]");
        assert!(
            has_detail || has_structural,
            "L2 should contain details or structural relationships"
        );
    }

    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn tier_token_counts_sum_approximately_to_total() -> Result<(), Box<dyn Error + Send + Sync>>
{
    let fixture = start_explanatory_fixture().await?;
    let mut client = fixture.query_client();

    let response = get_context_with_max_tier(&mut client, 0).await?;
    let rendered = response.rendered.expect("rendered should exist");

    let tier_total: u32 = rendered.tiers.iter().map(|t| t.token_count).sum();

    // Tier total won't exactly match rendered.token_count because
    // flat content has separators between sections. But it should be
    // in the same ballpark.
    assert!(tier_total > 0, "tier token total should be positive");
    assert!(
        (tier_total as f64) < (rendered.token_count as f64 * 1.5),
        "tier total ({tier_total}) should not vastly exceed flat total ({})",
        rendered.token_count
    );

    fixture.shutdown().await?;
    Ok(())
}
