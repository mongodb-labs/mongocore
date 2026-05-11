use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use mongocore::config::{CliArgs, Config};
use mongocore::connection::ConnectionPool;

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
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
        )
        .init();

    print_banner(&config);

    // Connect to MongoDB and detect capabilities
    let _pool = match ConnectionPool::connect(&config).await {
        Ok(pool) => {
            info!("MongoCore started successfully");
            pool
        }
        Err(e) => {
            error!("Failed to connect to MongoDB: {e}");
            std::process::exit(1);
        }
    };
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
