# Spinneret

Spinneret is an agent orchestrator and Tool API gateway. It dynamically spins up strict, capability-based WebAssembly (WASM) execution environments to execute untrusted third-party code. 

The low-level execution engine beneath Spinneret is called **Loom**. Built on top of `wasmtime` and the WASM Component Model, Loom provides the secure, zero-trust hypervisor layer.

## Features

* **Strict Capability-Based Security**: By default, sandboxes have no access to the host system.
* **Resource Metering**: CPU and Memory constraints (fuel limits and memory limits) are enforced per execution.
* **Granular Egress Control**: Custom domain allow-listing for outbound HTTP requests (`wasi:http`) through an interception proxy.
* **Worktree Mounting**: Controlled, isolated access to specific host directories via pre-opened directories.
* **Multi-tenancy Ready**: Designed to isolate execution states and provide environment variables dynamically per sandbox.
* **Agents as Tools**: Treat complex LLM agents as tools that can execute inside the sandbox via WASM Component Composition.

## Architecture

The project is structured into several interconnected crates:

* **`spinneret`**: The orchestrator HTTP/gRPC daemon. It receives JSON requests from management layers (like Arachne), pulls and verifies OCI artifacts, and coordinates the execution of tools.
* **`loom`**: The underlying WASI runtime and execution environment. It manages the Wasmtime engine, global component caching, and individual isolated execution sandboxes.
* **`loom-cli`**: A command-line interface for running and testing components locally via the Loom hypervisor.
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
