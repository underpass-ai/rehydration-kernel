use std::error::Error;
use std::time::Duration;

use async_nats::Client;
use testcontainers::{GenericImage, ImageExt, core::IntoContainerPort, runners::AsyncRunner};
use tokio::time::sleep;

use crate::agentic_support::agentic_debug::{debug_log, debug_log_value};

pub(crate) const NATS_IMAGE: &str = "docker.io/nats";
pub(crate) const NATS_TAG: &str = "2.10-alpine";
pub(crate) const NATS_INTERNAL_PORT: u16 = 4222;

pub(crate) async fn start_nats_container()
-> Result<testcontainers::ContainerAsync<GenericImage>, Box<dyn Error + Send + Sync>> {
    debug_log("starting nats container");
    Ok(GenericImage::new(NATS_IMAGE, NATS_TAG)
        .with_exposed_port(NATS_INTERNAL_PORT.tcp())
        .with_cmd(vec!["-js"])
        .start()
        .await?)
}

pub(crate) async fn connect_with_retry(url: &str) -> Result<Client, Box<dyn Error + Send + Sync>> {
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;

    for _ in 0..30 {
        match async_nats::connect(url).await {
            Ok(client) => {
                debug_log_value("connected to nats", url);
                return Ok(client);
            }
            Err(error) => {
                debug_log_value("nats connect retry error", &error);
                last_error = Some(Box::new(error));
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Err(last_error.expect("at least one connection attempt should fail"))
}
