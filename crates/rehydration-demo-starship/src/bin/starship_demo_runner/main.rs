use rehydration_demo_starship::{StarshipDemoConfig, run_starship_demo};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = StarshipDemoConfig::from_env()?;
    let summary = run_starship_demo(config).await?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}
