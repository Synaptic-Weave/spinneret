# Spinneret

Spinneret is a high-performance, strictly sandboxed execution engine for WebAssembly (WASM) components. Built on top of `wasmtime` and the WASM Component Model, it is designed for secure, multi-tenant execution of untrusted code with fine-grained control over system resources and capabilities.

## Features

* **Strict Capability-Based Security**: By default, sandboxes have no access to the host system.
* **Resource Metering**: CPU and Memory constraints (fuel limits and memory limits) are enforced per execution.
* **Granular Egress Control**: Custom domain allow-listing for outbound HTTP requests (`wasi:http`) through an interception proxy.
* **Worktree Mounting**: Controlled, isolated access to specific host directories via pre-opened directories.
* **Multi-tenancy Ready**: Designed to isolate execution states and provide environment variables dynamically per sandbox.

## Architecture

Spinneret is composed of a core execution engine and several interfaces:

* **`spinneret-core`**: The central WASI runtime and execution environment. Manages the Wasmtime engine, global component caching, and individual `Extrusion` sandboxes.
* **`spinneret-napi`** *(WIP)*: Node.js bindings via `napi-rs` to allow JavaScript/TypeScript orchestrators to launch sandboxes natively.
* **`spinneret-cli`** *(WIP)*: A command-line interface for running and testing components locally.
* **`spinneret-server`** *(WIP)*: An HTTP daemon that receives requests and dynamically spawns execution extrusions.
* **`hello-wasm`**: A sample WASM component used for integration testing.

## Getting Started

### Prerequisites

* Rust (latest stable)
* `cargo-component` for building WASM components (`cargo install cargo-component`)

### Building

```bash
cargo build --workspace
```

### Running Tests

```bash
cargo test --workspace
```
