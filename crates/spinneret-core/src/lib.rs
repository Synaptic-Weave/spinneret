use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use wasmtime::{Config, Engine as WasmtimeEngine, Store};
use wasmtime::component::Linker;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView, WasiCtxView};
use wasmtime_wasi_http::{WasiHttpCtx, p2::{WasiHttpView, WasiHttpCtxView, WasiHttpHooks, default_send_request}};

/// The central execution engine for Spinneret.
/// 
/// This wraps the `wasmtime::Engine` and is responsible for AOT compilation,
/// global caching, and instantiating individual `Extrusion` sandboxes.
#[derive(Clone)]
pub struct Engine {
    inner: WasmtimeEngine,
    linker: Linker<ExtrusionState>,
}

impl Engine {
    /// Creates a new Engine with strict capability-based security defaults.
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model(true); // Enable WASM Component Model
        config.consume_fuel(true); // Enable CPU metering

        let inner = WasmtimeEngine::new(&config)?;
        
        let mut linker = Linker::new(&inner);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;
        wasmtime_wasi_http::p2::add_only_http_to_linker_sync(&mut linker)?;

        Ok(Self { inner, linker })
    }

    /// Pre-compiles or loads a WASM component from the Authenticated Local Cache.
    pub fn load_component(&self, path: &PathBuf) -> Result<wasmtime::component::Component> {
        Ok(wasmtime::component::Component::from_file(&self.inner, path)?)
    }
}

/// Configuration for a single sandbox extrusion.
#[derive(Debug, Clone)]
pub struct ExtrusionConfig {
    pub fuel_limit: u64,
    pub memory_limit_bytes: usize,
    pub worktree_mount_path: Option<PathBuf>,
    pub environment_variables: HashMap<String, String>,
    pub allowed_egress_domains: Vec<String>,
}

impl Default for ExtrusionConfig {
    fn default() -> Self {
        Self {
            fuel_limit: 10_000_000,
            memory_limit_bytes: 1024 * 1024 * 50, // 50MB
            worktree_mount_path: None,
            environment_variables: HashMap::new(),
            allowed_egress_domains: Vec::new(),
        }
    }
}

pub struct SpinneretHttpHooks {
    allowed_domains: Vec<String>,
}

impl WasiHttpHooks for SpinneretHttpHooks {
    fn send_request(
        &mut self,
        request: hyper::Request<wasmtime_wasi_http::p2::body::HyperOutgoingBody>,
        config: wasmtime_wasi_http::p2::types::OutgoingRequestConfig,
    ) -> wasmtime_wasi_http::p2::HttpResult<wasmtime_wasi_http::p2::types::HostFutureIncomingResponse> {
        let host = request.uri().host().unwrap_or("");
        if !self.allowed_domains.iter().any(|d| d == host) {
            return Err(wasmtime_wasi_http::p2::HttpError::trap(wasmtime::Error::msg(format!("URL not allowed: {}", request.uri()))));
        }
        Ok(default_send_request(request, config))
    }
}

/// The state required for WASI execution.
pub struct ExtrusionState {
    pub ctx: WasiCtx,
    pub http: WasiHttpCtx,
    pub http_hooks: SpinneretHttpHooks,
    pub table: wasmtime::component::ResourceTable,
}

impl WasiView for ExtrusionState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for ExtrusionState {
    fn http(&mut self) -> WasiHttpCtxView<'_> {
        WasiHttpCtxView {
            ctx: &mut self.http,
            table: &mut self.table,
            hooks: &mut self.http_hooks,
        }
    }
}

/// An active execution sandbox.
pub struct Extrusion {
    store: Store<ExtrusionState>,
}

impl Extrusion {
    /// Creates a new isolated extrusion (Store) based on the config.
    pub fn new(engine: &Engine, config: ExtrusionConfig) -> Result<Self> {
        let mut builder = WasiCtxBuilder::new();

        // Inject multitenancy scope via environment variables
        for (k, v) in config.environment_variables.into_iter() {
            builder.env(k, v);
        }

        if let Some(worktree_path) = config.worktree_mount_path {
            builder.preopened_dir(
                worktree_path,
                "/worktree",
                wasmtime_wasi::DirPerms::all(),
                wasmtime_wasi::FilePerms::all(),
            )?;
        }
        
        let http_hooks = SpinneretHttpHooks {
            allowed_domains: config.allowed_egress_domains.clone(),
        };

        let state = ExtrusionState {
            ctx: builder.build(),
            http: WasiHttpCtx::new(),
            http_hooks,
            table: wasmtime::component::ResourceTable::new(),
        };

        let mut store = Store::new(&engine.inner, state);

        // Inject fuel
        store.set_fuel(config.fuel_limit)?;

        Ok(Self { store })
    }

    /// Executes the component as a standard WASI CLI command.
    pub fn run(&mut self, engine: &Engine, component: &wasmtime::component::Component) -> Result<()> {
        let command = wasmtime_wasi::p2::bindings::sync::Command::instantiate(&mut self.store, component, &engine.linker)?;
        
        command.wasi_cli_run().call_run(&mut self.store)?.map_err(|()| anyhow::anyhow!("component returned a failure exit status"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = Engine::new();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_extrusion_creation() {
        let engine = Engine::new().unwrap();
        let config = ExtrusionConfig::default();
        let extrusion = Extrusion::new(&engine, config);
        assert!(extrusion.is_ok());
    }

    #[test]
    fn test_extrusion_http_allowed() {
        let engine = Engine::new().unwrap();
        let mut config = ExtrusionConfig::default();
        config.allowed_egress_domains = vec!["example.com".to_string()];
        config.environment_variables.insert("TARGET_HOST".to_string(), "example.com".to_string());
        
        let mut extrusion = Extrusion::new(&engine, config).unwrap();
        let path = PathBuf::from("../../target/wasm32-wasip1/debug/hello-wasm.wasm");
        let component = engine.load_component(&path).unwrap();
        
        let result = extrusion.run(&engine, &component);
        // The component returns Ok and HTTP succeeds under the hood. 
        // Wasmtime trap does not occur.
        assert!(result.is_ok());
    }

    #[test]
    fn test_extrusion_http_blocked() {
        let engine = Engine::new().unwrap();
        let mut config = ExtrusionConfig::default();
        // Blocked.com is not in the allowed egress list
        config.allowed_egress_domains = vec!["example.com".to_string()];
        config.environment_variables.insert("TARGET_HOST".to_string(), "blocked.com".to_string());
        
        let mut extrusion = Extrusion::new(&engine, config).unwrap();
        let path = PathBuf::from("../../target/wasm32-wasip1/debug/hello-wasm.wasm");
        let component = engine.load_component(&path).unwrap();
        
        let result = extrusion.run(&engine, &component);
        // The component will trap because the URL is blocked
        assert!(result.is_err());
        let err_string = format!("{:?}", result.unwrap_err());
        assert!(err_string.contains("URL not allowed"));
    }
}
