# Architectural Suggestions for Spinneret & Loom

Based on a deep-dive review of the codebase and your subsequent comments, here are eight refined, high-impact architectural recommendations to elevate the **maintainability**, **extensibility**, and **stability** of the Spinneret hypervisor.

---

## 1. Concurrency-Safe, Asynchronous IPC Daemon

### Current Design
In `crates/spinneret/src/lib.rs`, the standard I/O IPC daemon loop (`run_daemon`) reads and processes incoming requests sequentially in a blocking loop:
```rust
for line_res in reader.lines() { ... }
```
If a component runs a long-running CPU computation (exhausting fuel) or hits a slow network timeout, **the entire daemon blocks**, preventing any other executions from being scheduled or managed in parallel.

### Recommendation
Refactor the standard I/O stream handler to run concurrently using **Tokio asynchronous channels** and task spawning:
* Read lines asynchronously using `tokio::io::BufReader`.
* Spawn a new tokio task (`tokio::spawn`) for each incoming request to decouple deserialization and scheduling.
* Use a multi-producer, single-consumer (mpsc) channel to funnel serialization responses back to a dedicated writer thread.
* **JSON-RPC 2.0 Compliance**: Introduce an `id` field in `LocalExecutionRequest` and `LocalExecutionResponse` to let callers associate asynchronous out-of-order responses with their corresponding requests.

---

## 2. Strict Standard I/O Isolation & Pipe Capture

### Current Design
Currently, guest WASM components run with `WasiCtxBuilder::new()`. Standard output and standard error configurations are not captured inside `Sandbox::new`. 
If a guest component outputs logs directly to stdout, they can either get lost or write directly to the host's stdout. In a standard I/O-based IPC setup, **uncontrolled guest prints will mix directly into the JSON-RPC pipe, corrupting the host-hypervisor communication protocol.**

### Recommendation
Explicitly isolate and capture standard streams for each `Sandbox`:
```rust
let stdout_pipe = wasmtime_wasi::pipe::MemoryWritableStream::new();
let stderr_pipe = wasmtime_wasi::pipe::MemoryWritableStream::new();

builder.stdout(stdout_pipe.clone());
builder.stderr(stderr_pipe.clone());
```
* Read these memory buffers after the execution completes.
* Expose `stdout` and `stderr` as clean, isolated string fields inside `LocalExecutionResponse`. This guarantees that guest console prints never interfere with the orchestrator protocol.

> [!TIP]
> **Robust Logging & Observability**: Isolating standard streams allows you to easily attach a dedicated logger per sandbox. For example, logs can be dynamically tagged with `[Sandbox-ID]`, written to rolling files, or forwarded directly to host-level observability tools without any risk of polluting the host-hypervisor IPC streams.

---

## 3. Structured Trap Classification & Polymorphic Error Handling

### Current Design
Currently, execution failures (fuel limits, memory limits, network bans) are returned back through `anyhow::Error` strings. The host-level orchestrator must resort to string parsing (e.g., `err_string.contains("URL not allowed")`) to classify why a sandbox terminated.

### Recommendation
Introduce a structured, serializable representation of execution traps and errors.

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "reason", content = "details")]
pub enum TrapCategory {
    FuelExhausted,
    MemoryExhausted { desired_bytes: usize, limit_bytes: usize },
    NetworkBlocked { url: String },
    GuestPanic { message: String },
    ExitCode { code: i32 },
}
```

> [!NOTE]
> **Dynamic Runtime Permission Requests**: By returning structured payloads like `NetworkBlocked { url: "api.github.com" }`, the host (Arachne or Electron) can cleanly intercept capability failures. Instead of failing outright, the host can request permissions from the user *during runtime* (e.g., a modal asking "Allow this tool to access api.github.com?"). If approved, the host can dynamically update the allowed egress list and transparently retry the execution.

> [!IMPORTANT]
> **Polymorphic Host-Side Class Hierarchy**: On the TypeScript (Arachne) host side, the serialized JSON error payload can be hydrated into a **true OOP class hierarchy** rather than a flat enum. This lets you encapsulate error-handling and recovery logic directly within dedicated subclasses:
>
> ```typescript
> abstract class SandboxError extends Error {
>     abstract handleRecovery(context: OrchestratorContext): Promise<boolean>;
> }
> 
> class EgressViolationError extends SandboxError {
>     constructor(public url: string, public domain: string) { super(); }
> 
>     async handleRecovery(context: OrchestratorContext): Promise<boolean> {
>         // Dynamic runtime permission prompt
>         const approved = await context.promptUserForPermission(this.url);
>         if (approved) {
>             context.config.allowedEgressDomains.push(this.domain);
>             return true; // Signals that execution can be retried
>         }
>         return false;
>     }
> }
> ```
> This completely decouples user interaction and recovery strategies from the low-level hypervisor execution loop.

---

## 4. Extensible "Host Capability" Registry

### Current Design
In `crates/loom/src/lib.rs`, host capability bindings are hardcoded within the `Engine` constructor:
```rust
wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
wasmtime_wasi_http::p2::add_only_http_to_linker_sync(&mut linker)?;
```
If the ecosystem expands to require custom host services (such as localized vector-DB search, fast key-value persistence, or telemetry sinks), developers must directly modify the hypervisor engine code.

### Recommendation
Decouple capabilities using a **Plugin Registry** design pattern. Define a `HostCapability` trait that can expose binding actions:

```rust
pub trait HostCapability {
    fn name(&self) -> &'static str;
    fn add_to_linker(&self, linker: &mut wasmtime::component::Linker<SandboxState>) -> anyhow::Result<()>;
}
```

This allows Spinneret to dynamically compile and link features on demand, making it exceptionally simple to add customized "Standard Library" APIs without altering the core runtime initialization routines.

---

## 5. Decoupled Component Composition & Resolution

### Current Design
WASM component composition (toolchaining) and dynamic loading are planned for Phase 2 but lack structural boundaries. Combining signature verification (`sigstore`), remote OCI downloading, AOT caching, and WASM linker composition inside a single flow can easily create tight coupling.

### Recommendation
Split the toolchain orchestration into three distinct, single-responsibility services:
1. **`RegistryClient`**: Solely responsible for interacting with registries and pulling artifacts.
2. **`SignatureVerifier` (using `sigstore`)**: Responsible for offline and transparency log crypto-checks.
3. **`ComponentComposer`**: Responsible for using `wtime-component-composition` or programmatic multi-component linker hooks to weave WASM interfaces (e.g., import-mapping `agent-worker` directly to the `http-client` export) prior to instantiation.

This separation ensures each block can be mocked, tested, and updated independently (e.g., allowing developers to test composition locally using directories without touching registries).

---

## 6. Idiomatic Project Modularization

### Current Design
Currently, `crates/loom/src/lib.rs` acts as a monolithic file housing multiple separate concerns (the global `Engine`, the ephemeral sandbox `Sandbox`, configuration structs, the runtime state `SandboxState`, and custom HTTP egress hooks `SpinneretHttpHooks`). 

### Recommendation
Refactor the single file into dedicated, single-responsibility files under an explicit module tree:
* **`src/engine.rs`**: Implements the compiled `Engine` and cache controls.
* **`src/sandbox.rs`**: Implements `Sandbox` (sandbox store) and `SandboxConfig`.
* **`src/state.rs`**: Implements the `SandboxState` wrapping the resources table and memory limits.
* **`src/hooks.rs`**: Implements standard HTTP interceptors (`SpinneretHttpHooks`).

Clean up **`src/lib.rs`** to only serve as the root interface, declaring modules and re-exporting them:
```rust
mod engine;
mod sandbox;
mod state;
mod hooks;

// Public re-exports to keep the integration backward-compatible
pub use engine::Engine;
pub use sandbox::{Sandbox, SandboxConfig};
pub use state::SandboxState;
pub use hooks::SpinneretHttpHooks;
```

This improves compile times, isolates module scopes, and matches standard Rust library engineering guidelines.

---

## 7. Decoupled Test Fixtures & Integration Testing

### Current Design
Currently, the inline test module inside `crates/loom/src/lib.rs` contains both unit-level tests (testing API configurations) and integration-level tests that rely on external pre-compiled WASM files (`hello-wasm.wasm`) located on the local disk.

### Recommendation
* **Inline Unit Tests**: Keep true unit tests that verify internal library configuration or isolated state builders within the inline `#[cfg(test)] mod tests` block in their respective files (e.g., inside `src/engine.rs`).
* **Extract External Fixtures**: Move all end-to-end integration tests that load and run Compiled `.wasm` files to a separate directory at `crates/loom/tests/sandbox_tests.rs`.
* **Benefit**: Ensures that running `cargo test --workspace` on a clean clone is reliable and environment-independent. It isolates the build dependency on WASM binaries entirely to the integration test execution boundary.

---

## 8. Semantic Alignment: Renaming `Extrusion` to `Sandbox`

### Current Design
Currently, the codebase uses the polymer-molding metaphor **"Extrusion"** (`Extrusion`, `ExtrusionConfig`, `ExtrusionState`) to represent the running WebAssembly execution environment. While highly descriptive of the startup transition, "extrusion" focuses heavily on the creation mechanism rather than the long-term execution lifecycle, resource boundaries, and isolation state.

### Recommendation
Rename all "Extrusion" concepts to **"Sandbox"** (e.g., `LoadSandbox`, `ExecuteSandbox`). This aligns the nomenclature with industry-standard terminology, improves developer ergonomics, and instantly maps the mental model of secure isolation:

| Old Nomenclature | New Nomenclature | Conceptual Role |
| :--- | :--- | :--- |
| `Extrusion` | `Sandbox` | The active isolated execution instance wrapping the Wasmtime store. |
| `ExtrusionConfig` | `SandboxConfig` | Holds fuel limits, memory limits, and domain allowlists for a run. |
| `ExtrusionState` | `SandboxState` | The localized state (WASI context, resource table) bound to the store. |
| `ExtrusionError` | `SandboxError` | Host-side error hierarchy for typed trap recovery. |

#### Code Comparison (Refactored Rust Public API):
```rust
// Initialize engine
let engine = Engine::new()?;

// Setup sandbox configuration
let mut config = SandboxConfig::default();
config.memory_limit_bytes = 50 * 1024 * 1024; // 50MB

// Instantiate and run the sandbox safely
let mut sandbox = Sandbox::new(&engine, config)?;
sandbox.run(&engine, &component)?;
```

This ensures the codebase is immediately intuitive, and maps perfectly to security audit terminology.
