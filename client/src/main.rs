use anyhow::Result;

use state::ComponentRunStates;
use tokio::io::{ReadHalf, SimplexStream, WriteHalf};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use wasmtime::{Store, component};
use wasmtime_wasi::{
    AsyncStdinStream, AsyncStdoutStream, ResourceTable, WasiCtxBuilder,
    bindings::Command,
    pipe::{AsyncReadStream, AsyncWriteStream},
};

mod monitored_stream;
mod state;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("info,{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Err(e) = tokio::fs::create_dir("logs").await {
        tracing::warn!("failed to create logs dir: {:?}", e);
    };

    let path = "target/wasm32-wasip2/release/server.wasm";
    let bytes = tokio::fs::read(&path).await?;

    let (client_read, write_to_client) = tokio::io::simplex(64);
    let (server_read, write_to_server) = tokio::io::simplex(64);

    let ct = tokio_util::sync::CancellationToken::new();

    let guard = ct.child_token();
    tokio::spawn(async move {
        tokio::select! {
            _ = guard.cancelled() => {
                tracing::info!("cancelled");
            }
            err = wasi_server(&bytes,server_read, write_to_client) => {
                tracing::error!("wasi_server: {:?}", err);
            }

        }

        anyhow::Ok(())
    });

    // let client_read = monitored_stream::MonitoredStream::new(client_read, (), "client_read");
    // let write_to_server =
    //     monitored_stream::MonitoredStream::new((), write_to_server, "write_to_server");

    let client =
        rmcp::service::serve_client_with_ct((), (client_read, write_to_server), ct.child_token())
            .await?;

    client.list_all_resources().await?;
    let tools = client.list_all_tools().await?;
    tracing::info!("tools: {:?}", tools);

    client.cancel().await?;
    ct.cancel();

    Ok(())
}

async fn wasi_server(
    bytes: &[u8],
    stdin: ReadHalf<SimplexStream>,
    stdout: WriteHalf<SimplexStream>,
) -> Result<()> {
    let mut config = wasmtime::Config::new();
    config.async_support(true);

    let engine = wasmtime::Engine::new(&config)?;

    let wasi_ctx = WasiCtxBuilder::new()
        .stdin(AsyncStdinStream::new(AsyncReadStream::new(stdin)))
        .stdout(AsyncStdoutStream::new(AsyncWriteStream::new(4096, stdout)))
        .preopened_dir(
            "logs",
            "logs",
            wasmtime_wasi::DirPerms::all(),
            wasmtime_wasi::FilePerms::all(),
        )?
        .build();

    let state = ComponentRunStates {
        wasi_ctx,
        resource_table: ResourceTable::new(),
    };
    let mut store = Store::new(&engine, state);

    let mut linker = component::Linker::new(&engine);
    wasmtime_wasi::add_to_linker_async(&mut linker)?;

    let component = component::Component::new(&engine, bytes)?;

    let instance = linker.instantiate_async(&mut store, &component).await?;
    let program_result = Command::new(&mut store, &instance)?
        .wasi_cli_run()
        .call_run(&mut store)
        .await?;

    let _ = program_result;

    Ok(())
}
