# Spinneret Roadmap

This roadmap outlines the development phases, planned API surfaces, and upcoming features for the Spinneret hypervisor.

## Phase 1: Core Execution & N-API Integration

The initial phase focuses on hardening the hypervisor execution sandbox, exposing the Node.js bindings, and establishing the integration testing lifecycle.

### The N-API Bridge (`spinneret-napi`)

The Rust core will expose strictly typed asynchronous interfaces to Node.js via `napi-rs`:

#### 1. Schema Extraction API
Used by the orchestrator to dynamically introspect an Orb without executing it.

* **Method**: `inspect_orb(ociUri: String) -> String`
* **Behavior**: Pulls the OCI artifact, cryptographically verifies it via Sigstore, and uses `wasmtime` component metadata APIs to parse the binary. It extracts the WIT interfaces, parameters, and return types, serializing them into a JSON string formatted for LLM Tool Schema generation.

#### 2. Extrusion API (Execution)
Used by the orchestrator to dynamically launch an isolated sandbox.

* **Method**: `extrude(ociUri: String, config: ExtrusionConfig) -> ExtrusionResult`
* **Config Payload (`ExtrusionConfig`)**:
  * `cpuLimit` (Integer)
  * `memoryLimitBytes` (Integer)
  * `worktreeMountPath` (String - absolute path)
  * `environmentVariables` (Record<String, String> - used for Instantiation Scoping)
  * `allowedEgressDomains` (Array - used for `wasi:http` allowlisting)
  * `allowedIdentity` (String - used for Sigstore policy enforcement)
* **Behavior**: Pulls and verifies the artifact (or loads from the Authenticated Local Cache), configures the `WasiCtx` and `Store`, links capabilities, executes the target function, and serializes the return payload or trapped error state back to Node.js.

### Testing & Validation

* **Rust Test Harness**: Pure Rust test suites (`cargo test`) to validate fuel exhaustion traps, memory cap traps, OIDC verification failures, and WASI environment injection independently of Node.js. *(Completed)*
* **Integration Script**: A TypeScript integration runner to validate the N-API boundary, proving that a TS script can invoke the `.node` module, pass complex `ExtrusionConfig` payloads, and safely catch timeout and security violation traps.

## Phase 2: Supply Chain Security & Component Composition

* **Sigstore Integration**: Implementation of the OCI pull mechanism and the strict Rust-native Sigstore verification layer (`sigstore` crate) to prevent Time-of-Check to Time-of-Use (TOCTOU) vulnerabilities.
* **Component Composition (Toolchaining)**: Enhancing the `Linker` to support dynamic dependency resolution between trusted and untrusted Orbs.
* **Authenticated Local Cache**: Implementation of the AOT compilation cache to optimize cold starts, using `Component::serialize()` and `Component::deserialize_file()` with SHA256 integrity checks.

## Phase 3: Advanced Capabilities

* **Asynchronous Host Callbacks**: Binding WASM imports to N-API threadsafe functions (`napi::threadsafe_function::ThreadsafeFunction`) to allow safe yielding to the Node.js event loop (e.g., publishing to a NATS message bus).
* **Daemonization (`spinneret-server`)**: Finalizing the standalone HTTP server daemon for networked sandbox provisioning.
* **CLI Refinement (`spinneret-cli`)**: Extending the command-line utility for local developers to test and debug Orbs with mocked environments and security constraints.
