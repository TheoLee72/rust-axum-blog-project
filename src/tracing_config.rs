use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize tracing with file and console logging
///
/// Sets up two separate logging layers:
/// 1. Console (stdout): INFO and above - visible during development
/// 2. File: DEBUG and above - detailed logs for debugging/production monitoring
///
/// **Important**: Must return WorkerGuard to keep the non-blocking file writer alive.
/// Without it, logs may not flush properly on shutdown.
///
/// # Returns
/// WorkerGuard that must be kept alive for the entire program lifetime.
/// Drop it in main() to trigger graceful shutdown and final flush of buffered logs.
pub fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    // Create a rolling file appender
    // daily() creates a new log file each day: blog_backend.log.2025-11-01, blog_backend.log.2025-11-02, etc.
    // Old files are retained but new writes go to the current day's file
    // Stored in ./logs directory (created automatically if doesn't exist)
    let file_appender = rolling::daily("./logs", "blog_backend.log");

    // Wrap the file appender in non-blocking writer
    // non_blocking() spawns a background thread that buffers writes
    // Benefits: Non-blocking I/O - doesn't slow down async tasks waiting for disk writes
    // guard: WorkerGuard that manages the background thread lifecycle
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    // Create file logging layer
    // with_writer(): Direct output to non-blocking file appender
    // with_ansi(false): Disable ANSI color codes (log files shouldn't contain escape sequences)
    // with_filter(): Only logs at DEBUG level and above go to file
    // This captures detailed debug info without cluttering console output
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking_file)
        .with_ansi(false)
        .with_filter(EnvFilter::new("debug"));

    // Create console logging layer
    // with_writer(std::io::stdout): Output directly to terminal/stdout
    // with_filter(): Only logs at INFO level and above - keeps console clean during development
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(false)
        .with_filter(EnvFilter::new("info"));

    // Initialize the global tracing subscriber
    // registry(): Creates the root subscriber that collects all trace events
    // with(console_layer): First layer registered - INFO+ to console
    // with(file_layer): Second layer registered - DEBUG+ to file
    // Both layers receive all events, but each filter controls what it actually logs
    // init(): Sets this as the global default subscriber for the entire program
    // Must be called exactly once at startup (panics if called twice)
    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .init();

    // Log that tracing is ready
    // This message goes to both console (INFO matches filter) and file (DEBUG > INFO)
    tracing::info!("Tracing initialized (console=INFO+, file=DEBUG+)");

    // Return the WorkerGuard
    // Caller must keep this alive with let _guard = init_tracing() in main()
    // When _guard is dropped (end of main), background writer thread shuts down gracefully
    // Ensures all buffered logs are flushed to disk before process exits
    guard
}
