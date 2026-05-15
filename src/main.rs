use clap::Parser;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use mongocore::analytics::AnalyticsCollector;
use mongocore::compiled::providers::sampling::McpSamplingProvider;
use mongocore::compiled::translator::CompiledQueryTranslator;
use mongocore::config::{CliArgs, Config};
use mongocore::connection::ConnectionPool;
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};
use mongocore::ingestion::{DirectoryWatcher, IngestionEngine};
use mongocore::mcp::{start_mcp_server, run_stdio_transport, McpHandler};
use mongocore::mcp::safety::SafetyConfig;
use mongocore::operations::Operations;
use mongocore::web_ui::start_web_ui_server;

#[tokio::main]
async fn main() {
    let cli = CliArgs::parse();

    let config = Config::load(&cli).unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {e}");
        std::process::exit(1);
    });

    // Initialize tracing/logging
    // In stdio mode, force logs to stderr so stdout stays clean for JSON-RPC
    let filter = if cli.stdio {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("warn"))
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&config.log_level))
    };

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
            let fmt_layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr);

            tracing_subscriber::registry()
                .with(filter)
                .with(fmt_layer)
                .with(otel_layer)
                .init();

            info!("OpenTelemetry tracing enabled, exporting to {}", config.otel_endpoint);
            Some(tracer_provider)
        } else {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .with_env_filter(filter)
                .init();
            None
        }
    };

    #[cfg(not(feature = "otel"))]
    {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(filter)
            .init();
    }

    // Connect to MongoDB and detect capabilities
    let pool = match ConnectionPool::connect(&config).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to MongoDB: {e}");
            std::process::exit(1);
        }
    };

    // Voyage AI API key (already resolved from TOML or env in config)
    let voyage_api_key = config.voyage_api_key.as_deref().map(|s| s.to_string());

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

    // Branch based on --stdio flag
    if cli.stdio {
        // stdio mode: run MCP over stdin/stdout, skip gRPC and HTTP servers
        let operations = Operations::new(pool.clone());
        let safety = SafetyConfig::default();

        // Create sampling channel for zero-config LLM via MCP host
        let (sampling_tx, sampling_rx) = tokio::sync::mpsc::channel(32);

        // Use configured LLM key if available, otherwise fall back to MCP sampling
        let llm_provider: Option<Box<dyn mongocore::compiled::providers::LlmProvider>> =
            if config.llm_api_key.is_some() || config.llm_gateway.is_some() {
                // Direct LLM provider would be created here from config
                // For now, fall through to sampling (the existing provider creation
                // logic is in the gRPC search handler — we'll unify later)
                None
            } else {
                Some(Box::new(McpSamplingProvider::new(sampling_tx.clone())))
            };

        let translator = Some(Arc::new(CompiledQueryTranslator::new(
            Some(pool.clone()),
            llm_provider,
            None,
            analytics.clone(),
        )));
        let voyage = voyage_api_key.map(|key| Arc::new(mongocore::voyage::client::VoyageClient::new(key)));
        let handler = Arc::new(McpHandler::new(
            operations,
            pool,
            safety,
            analytics,
            ingestion_engine,
            directory_watcher,
            translator,
            voyage,
            true,
        ));
        run_stdio_transport(handler, sampling_rx).await;
    } else {
        // Normal mode: print banner and start gRPC + HTTP MCP servers
        print_banner(&config);

        // Create shared compiled query translator (used by both MCP and Web UI)
        let llm_provider: Option<Box<dyn mongocore::compiled::providers::LlmProvider>> =
            if let Some(ref gw) = config.llm_gateway {
                Some(Box::new(mongocore::compiled::providers::gateway::GatewayProvider::new(
                    mongocore::compiled::providers::gateway::GatewayConfig {
                        base_url: gw.base_url.clone(),
                        api_key: gw.api_key.clone(),
                        auth_header: gw.auth_header.clone(),
                        model: gw.model.clone(),
                        provider_type: gw.provider_type.clone(),
                    },
                )))
            } else {
                None
            };

        let translator = Arc::new(CompiledQueryTranslator::new(
            Some(pool.clone()),
            llm_provider,
            None,
            analytics.clone(),
        ));

        // Start Web UI dashboard (if enabled)
        let _web_ui_handle = start_web_ui_server(
            &config,
            pool.clone(),
            analytics.clone(),
            Some(translator.clone()),
            ingestion_engine.clone(),
            directory_watcher.clone(),
        );

        // Start gRPC server
        let grpc_handle = start_grpc_server(
            pool.clone(),
            GrpcServerConfig {
                port: config.grpc_port,
                transport: config.transport.clone(),
                socket_path: config.socket_path.clone(),
                socket_permissions: config.socket_permissions,
                max_message_size: config.grpc_max_message_size,
                compression: config.grpc_compression.clone(),
                stream_idle_timeout_secs: config.stream_idle_timeout_secs,
                pipeline_timeout_secs: config.pipeline_timeout_secs,
                pipeline_max_concurrency: config.pipeline_max_concurrency,
            },
            voyage_api_key.as_deref(),
            analytics.clone(),
            ingestion_engine.clone(),
            directory_watcher.clone(),
        );

        // Start MCP server
        let mcp_handle = start_mcp_server(pool.clone(), config.mcp_port, voyage_api_key.as_deref(), analytics, ingestion_engine, directory_watcher, Some(translator));

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

    // Clean up UDS socket file on shutdown
    if config.transport != "tcp" {
        if std::path::Path::new(&config.socket_path).exists() {
            info!("Removing socket file: {}", config.socket_path);
            let _ = std::fs::remove_file(&config.socket_path);
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
    if config.web_ui_enabled {
        println!("  Web UI:    http://127.0.0.1:{}", config.web_ui_port);
    }
    println!("  Transport: {}", config.transport);
    if config.transport != "tcp" {
        println!("  UDS path:  {}", config.socket_path);
    }
    println!("  Log level: {}", config.log_level);
    println!();
}
