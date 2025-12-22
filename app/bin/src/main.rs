#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustbridge_stratum_service::run_from_config_path("config.yaml").await
}
