use anyhow::Result;
use rmcp::ServiceExt;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

mod llm;
mod wasi_io;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", "mcp-server.log");

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::TRACE.into()))
        .with_writer(file_appender)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .init();

    tracing::info!("Starting MCP server");

    let server = {
        let (stdin, stdout) = wasi_io::wasi_io();

        llm::LLM::new()
            .serve((stdin, stdout))
            .await
            .inspect_err(|e| {
                tracing::error!("serving error: {:?}", e);
            })?
    };

    server.waiting().await?;

    Ok(())
}
