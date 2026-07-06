# Spinneret Architecture

Spinneret is designed as a secure, capability-based WebAssembly (WASM) execution environment built around the WASM Component Model and `wasmtime`. It is specifically structured to execute untrusted third-party code in strictly confined sandboxes called "Extrusions".

This document outlines the high-level architecture and internal design of the project.

## Workspace Crates

The project is structured as a cargo workspace with several interdependent crates, each serving a distinct architectural role:

* **`spinneret-core`**: The foundational execution engine and runtime. It wraps `wasmtime` and handles the instantiation of `Extrusion` sandboxes, capability injection, and resource constraint enforcement.
* **`spinneret-napi`** *(Planned)*: The interoperability layer for Node.js. It uses `napi-rs` to expose the core execution engine as native bindings to a JavaScript/TypeScript orchestrator.
* **`spinneret-cli`** *(Planned)*: A command-line utility for local development. It allows developers to load and run WASM components through the engine from a terminal.
* **`spinneret-server`** *(Planned)*: A daemonized HTTP server that can receive payloads or component references over the network and spin up Extrusions dynamically.
* **`hello-wasm`**: A simple WASM component used for internal integration testing.

## Execution Model

The execution architecture is split into two primary paradigms: the global **Engine** and the localized **Extrusion**.

### 1. The Engine (`Engine`)

The `Engine` represents the global state of the runtime. Typically, an application will instantiate a single `Engine` and keep it alive for the duration of the process.

* **AOT Compilation & Caching**: The engine handles Ahead-Of-Time (AOT) compilation of WASM modules and manages the authenticated local cache. This ensures that a component is only compiled to native machine code once.
* **Linker Mapping**: The engine owns the `Linker`, which defines all the host capabilities (like `wasi:cli` and `wasi:http`) that components are allowed to import.

### 2. The Sandbox (`Extrusion`)

An `Extrusion` represents a single, isolated execution sandbox. Every time a component is executed, a new `Extrusion` is instantiated from the `Engine`.

* **The Store**: The `Extrusion` wraps a `wasmtime::Store`, which holds all the localized state, memory, and instances for that specific execution.
* **Extrusion Config**: When creating an `Extrusion`, an `ExtrusionConfig` is provided. This defines the exact constraints and capabilities injected into the sandbox for that run (e.g., fuel limits, environment variables).

## Security & Constraints

Spinneret is built on a "deny-by-default" capability model. Sandboxes start with zero access to the host machine and must have capabilities explicitly granted via the `ExtrusionConfig`.

### Resource Metering

* **CPU Fuel**: The engine operates on a deterministic fuel model. The `ExtrusionConfig` defines a `fuel_limit`. WASM instructions consume fuel, and if the limit is reached, the execution is immediately trapped and aborted.
* **Memory Limits**: Maximum heap allocations are constrained per-extrusion (`memory_limit_bytes`) to prevent memory exhaustion attacks.

### I/O & Egress Capabilities

* **WASI HTTP Allow-listing**: Spinneret implements a custom host proxy for `wasi:http` outbound requests via `WasiHttpHooks`. Outbound domain targets are checked against the `allowed_egress_domains` list in the `ExtrusionConfig`. If the domain is not explicitly allowed, the `send_request` call is intercepted and trapped.
* **Worktree Mounting**: Host filesystem access is blocked by default. The host can choose to mount specific, isolated directories into the WASM sandbox via WASI pre-opened directories using the `worktree_mount_path` configuration.
* **Environment Variables**: Multitenancy context and configurations are injected via dynamically constructed environment variables per execution.

## WASM Component Standards

Spinneret strictly adheres to the WASM Component Model (`wasm32-wasip1` / Preview 2). It utilizes standard WASI interfaces:
* `wasi:cli/run`: Used as the standard entrypoint for executing the component.
* `wasi:http/outgoing-handler`: Used by components to make egress network requests natively.
