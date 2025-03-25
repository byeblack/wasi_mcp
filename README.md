# WASI MCP Example Project

This is an example project demonstrating MCP (Message Channel Protocol) client and server implementation using the rmcp library. The project showcases how to compile the server with WASIp2 target and run it through the client using wasmtime.

## Project Structure

```
wasi_mcp/
├── client/         # MCP client implementation
├── server/         # MCP server implementation (compiled to WASIp2 target)
```

## Prerequisites

- Rust and Cargo (recommended installation via [rustup](https://rustup.rs/))
- WASIp2 target support: `rustup target add wasm32-wasip2`

## Build and Run

### Build the Server (WASIp2 target)

```bash
# Build WASIp2 target MCP Server
cargo build -r --target wasm32-wasip2 -p server
```

### Run the Client

The client will use the rmcp library and wasmtime to run the server:

```bash
# Run the client using rmcp and wasmtime
cargo run -p client
```
