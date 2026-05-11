use clap::Parser;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use mongocore::analytics::AnalyticsCollector;
use mongocore::config::{CliArgs, Config};
use mongocore::connection::ConnectionPool;
use mongocore::grpc::start_grpc_server;
use mongocore::mcp::start_mcp_server;

#[tokio::main]
async fn main() {
    let cli = CliArgs::parse();

    let config = Config::load(&cli).unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {e}");
        std::process::exit(1);
    });

    // Initialize tracing/logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    print_banner(&config);

    // Connect to MongoDB and detect capabilities
    let pool = match ConnectionPool::connect(&config).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to MongoDB: {e}");
            std::process::exit(1);
        }
    };

    // Resolve Voyage AI API key if configured
    let voyage_api_key = config
        .voyage_api_key_env
        .as_deref()
        .and_then(|env_var| std::env::var(env_var).ok());

    // Create analytics collector if enabled
    let analytics = if config.analytics_enabled {
        Some(Arc::new(AnalyticsCollector::new(config.analytics_buffer_size)))
    } else {
        None
    };

    // Start gRPC server
    let grpc_handle = start_grpc_server(pool.clone(), config.grpc_port, voyage_api_key.as_deref(), analytics);

    // Start MCP server
    let mcp_handle = start_mcp_server(pool.clone(), config.mcp_port);

    info!("MongoCore started successfully");

    // Wait for either server to exit (they run forever unless something fails)
    tokio::select! {
        result = grpc_handle => {
            match result {
                Ok(Ok(())) => info!("gRPC server shut down"),
                Ok(Err(e)) => error!("gRPC server error: {e}"),
                Err(e) => error!("gRPC server task panicked: {e}"),
            }
        }
        result = mcp_handle => {
            match result {
                Ok(()) => info!("MCP server shut down"),
                Err(e) => error!("MCP server task panicked: {e}"),
            }
        }
    }
}

fn print_banner(config: &Config) {
    println!(
        r#"
  __  __                          ____
 |  \/  | ___  _ __   __ _  ___ / ___|___  _ __ ___
 | |\/| |/ _ \| '_ \ / _` |/ _ \ |   / _ \| '__/ _ \
 | |  | | (_) | | | | (_| | (_) | |__| (_) | | |  __/
 |_|  |_|\___/|_| |_|\__, |\___/ \____\___/|_|  \___|
                      |___/
"#
    );
    println!("  MongoCore v{}", env!("CARGO_PKG_VERSION"));
    println!("  gRPC port: {}", config.grpc_port);
    println!("  MCP port:  {}", config.mcp_port);
    println!("  Log level: {}", config.log_level);
    println!();
}
