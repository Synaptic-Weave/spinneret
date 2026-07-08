# Spinneret Architecture

Spinneret is an orchestrator and Tool API Gateway designed to execute WebAssembly (WASM) components (Orbs) securely. It is part of a larger ecosystem involving three distinct layers: Arachne, Spinneret, and Loom.

## The 3-Tier Architecture

1. **Arachne (Management / The AI Brain)**
   * **Language**: TypeScript / Node.js
   * **Role**: The AI Controller and Management plane. Handles user conversations, prompt engineering, decides when a tool needs to be called, and sends a high-level request to Spinneret (`Execute Tool X with these JSON arguments`).
   * **Ignorant of**: WASM, sandboxing, registries, or how a tool is physically executed.

2. **Spinneret (The Orchestrator / Gateway)**
   * **Language**: Rust (HTTP/gRPC server)
   * **Role**: The Tool Control Plane and API Gateway. Maintains the Tool Registry, pulls and verifies OCI artifacts, and translates WASM `WIT` interfaces into JSON Tool Schemas for Arachne.
   * **Ignorant of**: LLM prompts, chat history, or how the hypervisor physically limits memory.

3. **Loom (The Execution Engine)**
   * **Language**: Rust (Library crate, `loom`)
   * **Role**: The Hypervisor and Sandbox. Wraps `wasmtime`, configures the Component Model, enforces strict capability-based security, meters resources (CPU/memory), and executes a single WASM function in an isolated `Extrusion`.
   * **Ignorant of**: LLMs, JSON schemas, or network registries.

## Zero-Trust Security & Boundary Model

Loom enforces strict capability-based security. Tools and Orbs are treated as entirely untrusted by default.

* **Network Deny-by-Default**: Untrusted Orbs are structurally denied general network access (`wasi:sockets` is not linked).
* **The Trusted Gateway Pattern**: Database access or sensitive operations must occur via *Trusted 1st-Party Orbs*. Untrusted tools must invoke Trusted tools via WASM-to-WASM Component Composition.
* **Instantiation Scoping (Multitenancy)**: The Host injects multitenancy context (e.g., `PARTITION_ID`) purely as localized WASI environment variables configured at the microsecond of the `Store` instantiation.
* **Egress Allowlisting (`wasi:http`)**: When built-in or trusted tenant Orbs are granted the `wasi:http/outgoing-handler` capability, the hypervisor intercepts all requests and enforces a strict, host-configured URL Allowlist.
* **Agents as Tools**: Complex agent execution loops, prompts, and behaviors can be compiled to WASM and passed to Loom as an Orb. The agent can use WASM Component Composition to invoke sub-tools entirely within the hypervisor, allowing Spinneret and Arachne to treat agents just like any other generic tool.

## Execution Model

The execution architecture in Loom is split into two primary paradigms: the global **Engine** and the localized **Extrusion**.

### 1. The Engine (`Engine`)

The `Engine` represents the global state of the runtime. 

* **Linker Mapping & Component Composition**: The engine owns the `Linker`, which defines all the host capabilities that components are allowed to import. The `Linker` supports dynamic dependency resolution (toolchaining). 

### 2. The Sandbox (`Extrusion`)

An `Extrusion` represents a single, isolated, ephemeral execution sandbox. 

* **The Store**: The `Extrusion` wraps a `wasmtime::Store`, which holds all the localized state, memory, and instances for that specific execution.
* **Extrusion Config**: When creating an `Extrusion`, an `ExtrusionConfig` defines the exact constraints and capabilities injected into the sandbox for that run.

## Resource Metering & Isolation

* **CPU Limits (Fuel)**: The engine utilizes `wasmtime`'s deterministic fuel consumption. 
* **Memory Limits**: Linear memory is hard-capped per `Store` (e.g., 50MB) to prevent memory exhaustion attacks.
* **Filesystem Isolation**: The `WasiCtxBuilder` mounts specific ephemeral host directories (e.g., a Git worktree) to virtual paths (e.g., `/workspace`). No broad OS filesystem access is permitted.

## Distribution & Supply Chain Verification

* **OCI-Compliant Packaging**: Orbs are distributed as standard OCI artifacts.
* **Sigstore Cosign Verification**: Orbs must be cryptographically signed using Keyless OIDC (Cosign).
* **TOCTOU Prevention**: To prevent Time-of-Check to Time-of-Use vulnerabilities, cryptographic verification happens entirely inside the Rust boundary using the `sigstore` crate.

## Authenticated Local Cache (AOT)

To guarantee sub-millisecond cold starts without compromising the TOCTOU security model, the dynamic loader implements a verified local cache.

## Local Deployment Mode: The Hosted Core

For local-first agent harnesses (such as desktop applications or developer-focused CLI environments), Spinneret supports running as a **Hosted Core** rather than a distributed cloud service. This topology eliminates remote OCI registry overhead and distributed networking latency.

Spinneret provides two local-first execution paradigms:

### 1. Embedded Library Mode (In-Process)
For desktop apps built with Rust-based frameworks like Tauri:
* Spinneret is linked directly as a library crate dependency.
* The host program communicates with the `HostedCore` API programmatically in-process, bypassing HTTP/gRPC completely.
* Eliminates network interface configuration and serialized transmission latency, enabling near-instantaneous execution.

### 2. Secure Local IPC Sidecar (Standard I/O JSON-RPC)
For desktop apps built with JavaScript/TypeScript (Electron) or Python (PyQt):
* Spinneret compiles as an executable sidecar binary and communicates via `stdin`/`stdout` Standard I/O using a JSON-RPC-style protocol.
* Avoids binding to TCP ports (such as `127.0.0.1:8080`), entirely preventing loopback network sniffing, port collisions, or operating system firewall warnings.

## Workspace Crates

* **`spinneret`**: The HTTP daemon capable of pulling Orbs, exposing JSON APIs to Arachne, and spinning up Extrusions via Loom.
* **`loom`**: The foundational WASI runtime and execution environment handling capabilities, limits, and the `Engine`/`Extrusion` implementations.
* **`loom-cli`**: A command-line utility for local development and running components.
* **`orbs/`**: The 1st-Party Native Tools (The Standard Library), deployed alongside the hypervisor for guaranteed component composition:
  * **`agent-worker`**: The core LLM loop. **NO** `wasi:http` capability. Forced to compose with network tools.
  * **`http-client`**: The Egress tool. **HAS** `wasi:http` capability and enforces the strict host allowlist.
  * **`hello-wasm`**: A sample WASM component used for internal integration testing.
