use clap::Parser;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use mongocore::analytics::AnalyticsCollector;
use mongocore::config::{CliArgs, Config};
use mongocore::connection::ConnectionPool;
use mongocore::grpc::start_grpc_server;
use mongocore::ingestion::{DirectoryWatcher, IngestionEngine};
use mongocore::mcp::start_mcp_server;

#[tokio::main]
async fn main() {
    let cli = CliArgs::parse();

    let config = Config::load(&cli).unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {e}");
        std::process::exit(1);
    });

    // Initialize tracing/logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    #[cfg(feature = "otel")]
    let _otel_provider = {
        if config.otel_enabled {
            use opentelemetry::global;
            use opentelemetry::trace::TracerProvider;
            use opentelemetry_otlp::WithExportConfig;
            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::util::SubscriberInitExt;

            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(&config.otel_endpoint)
                .build()
                .expect("Failed to build OTLP span exporter");

            let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .build();

            global::set_tracer_provider(tracer_provider.clone());

            let otel_layer = tracing_opentelemetry::layer()
                .with_tracer(tracer_provider.tracer(config.otel_service_name.clone()));
            let fmt_layer = tracing_subscriber::fmt::layer();

            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();

            info!("OpenTelemetry tracing enabled, exporting to {}", config.otel_endpoint);
            Some(tracer_provider)
        } else {
            tracing_subscriber::fmt().with_env_filter(filter).init();
            None
        }
    };

    #[cfg(not(feature = "otel"))]
    {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

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

    // Initialize ingestion engine if enabled
    let (ingestion_engine, directory_watcher) = if config.ingestion.enabled {
        let engine = Arc::new(IngestionEngine::new(pool.client(), "__mongocore"));
        let watcher = Arc::new(DirectoryWatcher::new(engine.clone(), pool.client().clone()));

        // Auto-start watch if configured
        if let Some(ref watch_config) = config.ingestion.watch {
            if watch_config.enabled.unwrap_or(false) {
                if let (Some(path), Some(database), Some(collection)) =
                    (&watch_config.path, &watch_config.database, &watch_config.collection)
                {
                    let wc = mongocore::ingestion::watch::WatchConfig {
                        path: std::path::PathBuf::from(path),
                        file_pattern: watch_config
                            .file_pattern
                            .clone()
                            .unwrap_or_else(|| "*.csv".to_string()),
                        database: database.clone(),
                        collection: collection.clone(),
                        conflict_strategy: parse_conflict_strategy(
                            watch_config.conflict_strategy.as_deref(),
                        ),
                        dedup_key: Vec::new(),
                        debounce_ms: config.ingestion.watch_debounce_ms,
                    };
                    match watcher.start_watch(wc).await {
                        Ok(id) => info!("Started directory watch: {}", id),
                        Err(e) => warn!("Failed to start directory watch: {}", e),
                    }
                }
            }
        }

        (Some(engine), Some(watcher))
    } else {
        (None, None)
    };

    // Start gRPC server
    let grpc_handle = start_grpc_server(
        pool.clone(),
        config.grpc_port,
        voyage_api_key.as_deref(),
        analytics.clone(),
        ingestion_engine.clone(),
        directory_watcher.clone(),
    );

    // Start MCP server
    let mcp_handle = start_mcp_server(pool.clone(), config.mcp_port, analytics, ingestion_engine, directory_watcher);

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

    #[cfg(feature = "otel")]
    {
        if let Some(provider) = _otel_provider {
            let _ = provider.shutdown();
        }
    }
}

fn parse_conflict_strategy(s: Option<&str>) -> mongocore::ingestion::ConflictStrategy {
    match s {
        Some("overwrite") => mongocore::ingestion::ConflictStrategy::Overwrite,
        Some("merge") => mongocore::ingestion::ConflictStrategy::Merge,
        _ => mongocore::ingestion::ConflictStrategy::Skip,
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
