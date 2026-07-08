# Product Requirements Document (PRD): Spinneret

## 1. Project Overview

**Product Name:** Spinneret

**Parent Ecosystem:** Arachne (Synaptic Weave)

**Role:** A Rust-based WebAssembly (WASM) hypervisor and execution engine, exposed as a native Node.js addon.

**Objective:** To provide an airtight, zero-trust, high-performance execution sandbox for untrusted `.wasm` Component binaries (Orbs). Spinneret guarantees strict resource metering (CPU/Memory), cryptographic supply chain verification, and robust WASI Preview 2 compliance, while seamlessly integrating with Arachne's TypeScript orchestration layer via N-API.

## 2. Core Architecture

**Hypervisor Engine:** Rust leveraging the `wasmtime` crate (the official Bytecode Alliance runtime).

**Component Standard:** WebAssembly Component Model and WebAssembly System Interface (WASI Preview 2).

**Host Bridge:** `napi-rs` (compiling the Rust engine into a native `.node` binary for TypeScript).

**Orchestrator Environment:** Node.js (TypeScript) executing the native bindings.

## 3. Security & Boundary Model (Zero-Trust)

Spinneret enforces capability-based security. Tools are treated as entirely untrusted by default.

**Network Deny-by-Default:** Untrusted Orbs are structurally denied network access (`wasi:sockets` is not linked). They cannot bypass host rules to access databases or external APIs.

**The Trusted Gateway Pattern:** Database access must occur via *Trusted 1st-Party Orbs* (e.g., a Vector DB Orb). Untrusted tools must invoke Trusted tools via WASM-to-WASM Component Composition.

**Instantiation Scoping (Multitenancy):** The Host injects multitenancy context (e.g., `PARTITION_ID`) purely as localized WASI environment variables configured at the microsecond of the `Store` instantiation. Trusted Orbs read this environment state to securely scope outbound requests, making it impossible for an upstream untrusted tool to override the tenant boundary.

**Egress Allowlisting (`wasi:http`):** When built-in or trusted tenant Orbs (like an LLM Inference tool) are granted the `wasi:http/outgoing-handler` capability, the Rust hypervisor intercepts all requests and enforces a strict, host-configured URL Allowlist using **Fully Qualified Domain Names (FQDNs)**. Exfiltration attempts instantly trap the execution.

## 4. Distribution & Cryptographic Trust (Supply Chain)

**OCI-Compliant Packaging:** Orbs are distributed as standard OCI artifacts, pullable from standard registries (e.g., GHCR, Docker Hub).

**Registry Authentication:** The N-API bridge will accept registry credentials (e.g., standard Docker config or Bearer tokens) to allow pulling private Orbs from standard OCI registries.

**Sigstore Cosign Verification:** Orbs must be cryptographically signed using Keyless OIDC (Cosign).

**TOCTOU Prevention (Rust-Native Verification):** To prevent Time-of-Check to Time-of-Use vulnerabilities, cryptographic verification must happen entirely inside the Rust hypervisor using the `sigstore` crate. Arachne simply passes the OCI URI across the N-API boundary; Spinneret pulls the artifact, verifies the signature against the Rekor transparency log, checks the tenant identity policy, and compiles the code within the same locked memory space.

## 5. Functional Requirements: Advanced Execution

**Authenticated Local Cache (AOT):** To guarantee sub-millisecond cold starts without compromising the TOCTOU security model, the dynamic loader implements a verified local cache. *(Note: Cache Eviction/Garbage Collection is deferred to a future phase).*
- *Cold Start:* On first request of an OCI URI, Spinneret pulls, cryptographically verifies, compiles via Cranelift, and uses `Component::serialize()` to cache the compiled machine code in a highly restricted host directory (e.g., `/var/lib/spinneret/cache/`).
- *Hot Start:* On subsequent executions, Spinneret performs a rapid SHA256 integrity check on the local file and uses `Component::deserialize_file()` to map the executable directly into memory, bypassing compilation completely.

- **Component Composition (Toolchaining):** The `Linker` must support dynamic dependency resolution entirely within the Rust boundary. When Spinneret loads an Orb, it must parse the bundle's specfile for dependencies. For any nested dependency (e.g., Tool A requires Tool B), Spinneret must recursively pull, cryptographically verify, and link the required OCI artifacts.
  - **Dependency Caching:** To prevent redundant network calls and latency, Spinneret must utilize the Authenticated Local Cache for resolved dependencies. On a cache miss, it retrieves and compiles the dependency; on a cache hit, it deserializes the executable from disk directly into memory. Fuel and memory limits apply globally to the entire resulting execution chain.
- **Asynchronous Host Callbacks:** For capabilities requiring Node.js event loop interaction (e.g., yielding to the NATS bus), the N-API bridge must use `napi::threadsafe_function::ThreadsafeFunction`. The Rust `Linker` binds WASM imports to these callbacks, allowing the isolated thread to yield events safely without blocking the hypervisor.

## 6. Functional Requirements: The Rust Hypervisor Core

**Global Engine:** The system must instantiate a global, thread-safe `wasmtime::Engine` configured for the WebAssembly Component Model.

**Isolated Execution:** It must dynamically load `.wasm` binaries into isolated, ephemeral `wasmtime::Store` instances.

**Resource Metering:**
- **CPU Limits (Fuel):** The engine must utilize `wasmtime`'s fuel consumption. Every execution receives a strict instruction limit. Exceeding this limit (e.g., an infinite loop) must instantly trap the instance and return a timeout error to the host.
- **Memory Caps:** Linear memory must be hard-capped per `Store` (e.g., 50MB). Violations must trigger an immediate trap.

- **Filesystem Isolation:** The `WasiCtxBuilder` must be used to mount specific ephemeral host directories (e.g., a Git worktree) to virtual paths (e.g., `/workspace`). No broad OS filesystem access is permitted.

## 7. Functional Requirements: The N-API Bridge

The Rust core must expose the following strictly typed asynchronous interfaces to Node.js via `napi-rs`. Inputs from Node.js and outputs from Rust will use standard JSON serialization, with Spinneret translating between JSON and the underlying WIT Component types.

### 7.1. Schema Extraction API

**Method:** `inspect_orb(ociUri: String, registryAuth?: AuthConfig) -> String`
**Behavior:** Pulls the OCI artifact, verifies it, and uses `wasmtime` component metadata APIs to parse the binary *without* executing it. Extracts the WIT interfaces, parameters, and return types, serializing them into a JSON string formatted for LLM Tool Schema generation.

### 7.2. Extrusion API (Execution)

**Method:** `extrude(ociUri: String, config: ExtrusionConfig) -> ExtrusionResult`
**Config Payload (`ExtrusionConfig`):**
- `cpuLimit` (Integer)
- `memoryLimitBytes` (Integer)
- `worktreeMountPath` (String - absolute path)
- `environmentVariables` (Record<String, String> - used for Instantiation Scoping)
- `allowedEgressDomains` (Array of Strings - strictly FQDNs for `wasi:http` allowlisting)
- `allowedIdentity` (String - used for Sigstore policy enforcement)
- `registryAuth` (Object - Credentials for private OCI registries)

**Behavior:** Pulls/Verifies or loads from the Authenticated Cache, configures the `WasiCtx` and `Store`, links capabilities, executes the target function, and serializes the return payload or trapped error state back to Node.js.

## 8. Development & Testing (Phase 1)

**Rust Test Harness:** A pure Rust test suite (`cargo test`) must validate fuel exhaustion traps, memory cap traps, OIDC verification failures, and WASI environment injection independently of Node.js.
**Integration Script:** A TypeScript integration runner must be included to validate the N-API boundary, proving that a TS script can invoke the `.node` module, pass complex `ExtrusionConfig` payloads, and safely catch timeout and security violation traps.

## 9. Out of Scope (Phase 1)

Cache Eviction / Garbage Collection for the Authenticated Local Cache.

LLM agent state logic, context window management, and prompt engineering.

Arachne's Unified Tool Registry schema mapping (Spinneret provides the raw JSON; Arachne formats it).

Cluster orchestration and NATS distributed networking logic.
