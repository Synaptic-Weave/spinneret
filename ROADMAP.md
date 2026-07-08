# Spinneret Roadmap

This roadmap outlines the development phases, planned API surfaces, and upcoming features for the Spinneret orchestrator and the Loom hypervisor.

## Phase 1: Core Execution & Daemon Integration

The initial phase focuses on hardening the `loom` hypervisor execution sandbox, building the `spinneret` library and daemon to support local-first desktop integration (the Hosted Core), and establishing the tool execution APIs.

### Local & Library Execution (Hosted Core)
* **Embedded Library (`lib.rs`)**: Package programmatic in-memory execution bindings for Rust hosts (like Tauri desktop apps). *(Completed)*
* **Secure Stdin/Stdout Sidecar IPC**: Enable Electron (TS) and Python to run the Spinneret binary as a secure, zero-port-conflict child process using a JSON-RPC-style stdin/stdout streaming interface. *(Completed)*

### The Spinneret Tool API

Spinneret will expose strictly typed REST/gRPC interfaces to Arachne:

#### 1. Schema Extraction API
Used by Arachne to dynamically introspect an Orb without executing it.

* **Method**: `inspect_orb(ociUri: String) -> String`
* **Behavior**: Pulls the OCI artifact, cryptographically verifies it via Sigstore, and uses `wasmtime` component metadata APIs to parse the binary. It extracts the WIT interfaces, parameters, and return types, serializing them into a JSON string formatted for LLM Tool Schema generation.

#### 2. Extrusion API (Execution)
Used by Arachne to dynamically launch an isolated sandbox.

* **Method**: `extrude(ociUri: String, payload: JsonPayload, config: ExtrusionConfig) -> ExtrusionResult`
* **Config Payload (`ExtrusionConfig`)**:
  * `cpuLimit` (Integer)
  * `memoryLimitBytes` (Integer)
  * `worktreeMountPath` (String - absolute path)
  * `environmentVariables` (Record<String, String> - used for Instantiation Scoping)
  * `allowedEgressDomains` (Array - used for `wasi:http` allowlisting)
  * `allowedIdentity` (String - used for Sigstore policy enforcement)
* **Behavior**: Pulls and verifies the artifact (or loads from the Authenticated Local Cache), configures the Loom `Extrusion`, parses the incoming JSON payload into WASM arguments, executes the target function, and serializes the return payload or trapped error state back to JSON for Arachne.

### Testing & Validation

* **Loom Test Harness**: Pure Rust test suites (`cargo test`) to validate fuel exhaustion traps, memory cap traps, OIDC verification failures, and WASI environment injection. *(Completed)*
* **Spinneret Daemon Tests**: End-to-end API testing of the HTTP server, proving it can receive a JSON Tool execution request and route it through Loom successfully.

## Phase 2: Supply Chain Security & Component Composition

* **Sigstore Integration**: Implementation of the OCI pull mechanism and the strict Rust-native Sigstore verification layer (`sigstore` crate) in Spinneret to prevent Time-of-Check to Time-of-Use (TOCTOU) vulnerabilities.
* **Component Composition (Toolchaining)**: Enhancing the Loom `Linker` to support dynamic dependency resolution between trusted and untrusted Orbs.
* **Agents as Tools**: Using the component composition layer to compile complex LLM agents into WASM components, allowing Spinneret to pass the agent to Loom alongside its sub-tools for hyper-fast execution isolated from the main orchestrator memory.
* **Authenticated Local Cache**: Implementation of the AOT compilation cache to optimize cold starts, using `Component::serialize()` and `Component::deserialize_file()` with SHA256 integrity checks.

## Phase 3: Advanced Capabilities

* **Daemonization (`spinneret`)**: Hardening the standalone HTTP server daemon for networked sandbox provisioning, adding telemetry and observability (tracing/Prometheus).
* **CLI Refinement (`loom-cli`)**: Extending the command-line utility for local developers to test and debug Orbs with mocked environments and security constraints before uploading them to registries.
