# Spinneret Architecture

Spinneret is designed as a secure, capability-based WebAssembly (WASM) hypervisor and execution engine built around the WASM Component Model and `wasmtime`. It is specifically structured to execute untrusted third-party code (Orbs) in strictly confined sandboxes called "Extrusions".

Spinneret guarantees strict resource metering (CPU/Memory), cryptographic supply chain verification, and robust WASI Preview 2 compliance, while seamlessly integrating with a TypeScript orchestration layer (Arachne) via N-API.

## Zero-Trust Security & Boundary Model

Spinneret enforces strict capability-based security. Tools and Orbs are treated as entirely untrusted by default.

* **Network Deny-by-Default**: Untrusted Orbs are structurally denied general network access (`wasi:sockets` is not linked). They cannot bypass host rules to access databases or external APIs directly.
* **The Trusted Gateway Pattern**: Database access or sensitive operations must occur via *Trusted 1st-Party Orbs* (e.g., a Vector DB Orb). Untrusted tools must invoke Trusted tools via WASM-to-WASM Component Composition.
* **Instantiation Scoping (Multitenancy)**: The Host injects multitenancy context (e.g., `PARTITION_ID`) purely as localized WASI environment variables configured at the microsecond of the `Store` instantiation. Trusted Orbs read this environment state to securely scope outbound requests, making it impossible for an upstream untrusted tool to override the tenant boundary.
* **Egress Allowlisting (`wasi:http`)**: When built-in or trusted tenant Orbs are granted the `wasi:http/outgoing-handler` capability, the Rust hypervisor intercepts all requests and enforces a strict, host-configured URL Allowlist. Exfiltration attempts instantly trap the execution.

## Execution Model

The execution architecture is split into two primary paradigms: the global **Engine** and the localized **Extrusion**.

### 1. The Engine (`Engine`)

The `Engine` represents the global state of the runtime. Typically, an application will instantiate a single `Engine` and keep it alive for the duration of the process.

* **Linker Mapping & Component Composition**: The engine owns the `Linker`, which defines all the host capabilities that components are allowed to import. The `Linker` supports dynamic dependency resolution (toolchaining). If Tool A imports an interface belonging to Tool B, the engine must instantiate both within the same `Store` and fulfill the import natively.
* **Asynchronous Host Callbacks**: For capabilities requiring Node.js event loop interaction, the N-API bridge uses `napi::threadsafe_function::ThreadsafeFunction`. The Rust `Linker` binds WASM imports to these callbacks, allowing the isolated thread to yield events safely without blocking the hypervisor.

### 2. The Sandbox (`Extrusion`)

An `Extrusion` represents a single, isolated, ephemeral execution sandbox. Every time a component is executed, a new `Extrusion` is instantiated from the `Engine`.

* **The Store**: The `Extrusion` wraps a `wasmtime::Store`, which holds all the localized state, memory, and instances for that specific execution.
* **Extrusion Config**: When creating an `Extrusion`, an `ExtrusionConfig` defines the exact constraints and capabilities injected into the sandbox for that run (fuel limits, memory caps, identity policies).

## Resource Metering & Isolation

* **CPU Limits (Fuel)**: The engine utilizes `wasmtime`'s deterministic fuel consumption. The `ExtrusionConfig` defines a `fuel_limit`. Exceeding this limit instantly traps the instance and returns a timeout error to the host.
* **Memory Limits**: Linear memory is hard-capped per `Store` (e.g., 50MB) to prevent memory exhaustion attacks.
* **Filesystem Isolation**: The `WasiCtxBuilder` mounts specific ephemeral host directories (e.g., a Git worktree) to virtual paths (e.g., `/workspace`). No broad OS filesystem access is permitted.

## Distribution & Supply Chain Verification

* **OCI-Compliant Packaging**: Orbs are distributed as standard OCI artifacts, pullable from standard registries (e.g., GHCR, Docker Hub).
* **Sigstore Cosign Verification**: Orbs must be cryptographically signed using Keyless OIDC (Cosign).
* **TOCTOU Prevention**: To prevent Time-of-Check to Time-of-Use vulnerabilities, cryptographic verification happens entirely inside the Rust hypervisor using the `sigstore` crate. The OCI URI is passed across the N-API boundary; Spinneret pulls the artifact, verifies the signature against the Rekor transparency log, checks the tenant identity policy, and compiles the code within the same locked memory space.

## Authenticated Local Cache (AOT)

To guarantee sub-millisecond cold starts without compromising the TOCTOU security model, the dynamic loader implements a verified local cache.

* **Cold Start**: On the first request of an OCI URI, Spinneret pulls, cryptographically verifies, compiles via Cranelift, and uses `Component::serialize()` to cache the compiled machine code in a highly restricted host directory.
* **Hot Start**: On subsequent executions, Spinneret performs a rapid SHA256 integrity check on the local file and uses `Component::deserialize_file()` to map the executable directly into memory, bypassing compilation completely.

## Workspace Crates

* **`spinneret-core`**: The foundational WASI runtime and execution environment handling capabilities, limits, and the `Engine`/`Extrusion` implementations.
* **`spinneret-napi`**: Node.js bindings via `napi-rs` exposing the core execution engine natively to the TypeScript orchestrator.
* **`spinneret-cli`**: A command-line utility for local development and running components.
* **`spinneret-server`**: An HTTP daemon capable of spinning up Extrusions dynamically over the network.
* **`hello-wasm`**: A simple WASM component used for internal integration testing.
