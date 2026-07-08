use anyhow::Result;
use loom::{Engine, Extrusion, ExtrusionConfig};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Programmatic interface for integrating Spinneret into a local desktop harness (like Tauri).
pub struct HostedCore {
    engine: Engine,
    local_orbs_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalExecutionRequest {
    /// The name of the Orb WASM component (e.g. "agent-worker" or "http-client")
    pub orb_name: String,
    /// Arbitrary input parameters to pass to the execution sandbox
    pub input_json: serde_json::Value,
    /// Resource limit for CPU (Wasmtime fuel limit)
    pub fuel_limit: Option<u64>,
    /// Resource limit for linear memory (in bytes)
    pub memory_limit: Option<usize>,
    /// Path to a local host directory to pre-open and mount under `/worktree`
    pub workspace_path: Option<PathBuf>,
    /// List of strictly allowed FQDNs for outgoing HTTP requests
    pub allowed_domains: Option<Vec<String>>,
    /// Key-value pairs to inject as environment variables
    pub env_vars: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalExecutionResponse {
    /// The result of the execution serialized as JSON
    pub output_json: serde_json::Value,
    /// Total fuel consumed during execution (0 if not supported or timed out)
    pub fuel_consumed: u64,
}

impl HostedCore {
    /// Initialize the Hosted Core with a set of local paths to search for pre-compiled or cached Orbs.
    pub fn init(search_dirs: Vec<PathBuf>) -> Result<Self> {
        let engine = Engine::new()?;
        Ok(Self {
            engine,
            local_orbs_dirs: search_dirs,
        })
    }

    /// Resolve an Orb's filename locally across standard search directories.
    pub fn resolve_orb_path(&self, orb_name: &str) -> Option<PathBuf> {
        // Look for orb_name.wasm in our search paths
        for dir in &self.local_orbs_dirs {
            let path = dir.join(format!("{}.wasm", orb_name));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    /// Extrudes a local sandbox and executes an Orb programmatically.
    pub fn execute(&self, req: LocalExecutionRequest) -> Result<LocalExecutionResponse> {
        // Resolve local path of the Orb WASM component
        let orb_path = self.resolve_orb_path(&req.orb_name)
            .ok_or_else(|| anyhow::anyhow!("Orb '{}' not found in search directories", req.orb_name))?;

        // Load the WASM component in memory
        let component = self.engine.load_component(&orb_path)?;

        // Prepare the Loom extrusion config
        let mut config = ExtrusionConfig::default();
        if let Some(fuel) = req.fuel_limit {
            config.fuel_limit = fuel;
        }
        if let Some(mem) = req.memory_limit {
            config.memory_limit_bytes = mem;
        }
        config.worktree_mount_path = req.workspace_path;
        if let Some(domains) = req.allowed_domains {
            config.allowed_egress_domains = domains;
        }

        // Pass down environment variables (such as configuration and input payload context)
        let mut env_vars = req.env_vars.unwrap_or_default();
        
        // Serialize input JSON as an environment variable to pass to the WASI component
        env_vars.insert("SPINNERET_INPUT".to_string(), req.input_json.to_string());
        config.environment_variables = env_vars;

        // Create the extrusion sandbox
        let mut extrusion = Extrusion::new(&self.engine, config)?;

        // Execute inside the sandbox
        // Since we run standard WASI command-run, we capture success/failure.
        extrusion.run(&self.engine, &component)?;

        // Stubbed response for execution metrics
        // In subsequent phases, we can read stdout or extract output states 
        // and fetch precise fuel consumption metrics from the store.
        Ok(LocalExecutionResponse {
            output_json: serde_json::json!({
                "status": "success",
                "message": format!("Executed Orb '{}' programmatically.", req.orb_name)
            }),
            fuel_consumed: 0, 
        })
    }
}


/// Runs the secure local IPC daemon loop, reading from `reader` and writing to `writer`.
pub fn run_daemon<R, W>(core: &HostedCore, reader: R, mut writer: W) -> Result<()>
where
    R: std::io::BufRead,
    W: std::io::Write,
{
    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break, // EOF or stream error
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Attempt to parse the request
        let req: LocalExecutionRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = serde_json::json!({
                    "error": format!("Invalid JSON request: {}", e)
                });
                let _ = writeln!(writer, "{}", err_resp);
                let _ = writer.flush();
                continue;
            }
        };

        // Execute the request via HostedCore
        match core.execute(req) {
            Ok(resp) => {
                let resp_str = match serde_json::to_string(&resp) {
                    Ok(s) => s,
                    Err(e) => {
                        let err_resp = serde_json::json!({
                            "error": format!("Failed to serialize response: {}", e)
                        });
                        let _ = writeln!(writer, "{}", err_resp);
                        let _ = writer.flush();
                        continue;
                    }
                };
                let _ = writeln!(writer, "{}", resp_str);
            }
            Err(e) => {
                let err_resp = serde_json::json!({
                    "error": format!("Execution failed: {}", e)
                });
                let _ = writeln!(writer, "{}", err_resp);
            }
        }
        let _ = writer.flush();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_hosted_core_init() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core = HostedCore::init(vec![temp_dir.path().to_path_buf()]);
        assert!(core.is_ok());
    }

    #[test]
    fn test_resolve_orb_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dummy_wasm = temp_dir.path().join("dummy-orb.wasm");
        fs::write(&dummy_wasm, "empty").unwrap();

        let core = HostedCore::init(vec![temp_dir.path().to_path_buf()]).unwrap();
        let resolved = core.resolve_orb_path("dummy-orb");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap(), dummy_wasm);

        let missing = core.resolve_orb_path("missing-orb");
        assert!(missing.is_none());
    }

    #[test]
    fn test_hosted_core_execute_missing_orb() {
        let temp_dir = tempfile::tempdir().unwrap();
        let core = HostedCore::init(vec![temp_dir.path().to_path_buf()]).unwrap();
        let req = LocalExecutionRequest {
            orb_name: "missing-orb".to_string(),
            input_json: serde_json::json!({}),
            fuel_limit: None,
            memory_limit: None,
            workspace_path: None,
            allowed_domains: None,
            env_vars: None,
        };
        let res = core.execute(req);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Orb 'missing-orb' not found"));
    }

    #[test]
    fn test_hosted_core_execute_success() {
        // Point HostedCore search path to target directory containing hello-wasm.wasm
        let target_dir = PathBuf::from("../../target/wasm32-wasip1/debug");
        let core = HostedCore::init(vec![target_dir]).unwrap();
        
        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_MODE".to_string(), "env".to_string());
        env_vars.insert("MY_VAR".to_string(), "MY_VALUE".to_string());
        
        let req = LocalExecutionRequest {
            orb_name: "hello-wasm".to_string(),
            input_json: serde_json::json!({"some_key": "some_value"}),
            fuel_limit: Some(1_000_000),
            memory_limit: Some(50 * 1024 * 1024),
            workspace_path: None,
            allowed_domains: None,
            env_vars: Some(env_vars),
        };
        
        let res = core.execute(req);
        assert!(res.is_ok());
        let resp = res.unwrap();
        assert_eq!(resp.output_json["status"], "success");
    }

    #[test]
    fn test_hosted_core_execute_blocked_domain() {
        let target_dir = PathBuf::from("../../target/wasm32-wasip1/debug");
        let core = HostedCore::init(vec![target_dir]).unwrap();
        
        let mut env_vars = HashMap::new();
        env_vars.insert("TARGET_HOST".to_string(), "blocked.com".to_string());
        
        let req = LocalExecutionRequest {
            orb_name: "hello-wasm".to_string(),
            input_json: serde_json::json!({}),
            fuel_limit: Some(10_000_000),
            memory_limit: Some(50 * 1024 * 1024),
            workspace_path: None,
            allowed_domains: Some(vec!["example.com".to_string()]),
            env_vars: Some(env_vars),
        };
        
        let res = core.execute(req);
        assert!(res.is_err());
        let err_msg = format!("{:?}", res.unwrap_err());
        assert!(err_msg.contains("URL not allowed"));
    }

    #[test]
    fn test_daemon_ipc_valid() {
        let target_dir = PathBuf::from("../../target/wasm32-wasip1/debug");
        let core = HostedCore::init(vec![target_dir]).unwrap();

        let mut env_vars = HashMap::new();
        env_vars.insert("TEST_MODE".to_string(), "env".to_string());
        env_vars.insert("MY_VAR".to_string(), "MY_VALUE".to_string());

        let req = LocalExecutionRequest {
            orb_name: "hello-wasm".to_string(),
            input_json: serde_json::json!({}),
            fuel_limit: Some(1_000_000),
            memory_limit: Some(50 * 1024 * 1024),
            workspace_path: None,
            allowed_domains: None,
            env_vars: Some(env_vars),
        };

        let req_json = serde_json::to_string(&req).unwrap();
        let input_data = format!("{}\n", req_json);
        let reader = std::io::Cursor::new(input_data);
        let mut writer = Vec::new();

        let res = run_daemon(&core, reader, &mut writer);
        assert!(res.is_ok());

        let output_str = String::from_utf8(writer).unwrap();
        let resp: serde_json::Value = serde_json::from_str(&output_str).unwrap();
        assert_eq!(resp["output_json"]["status"], "success");
    }

    #[test]
    fn test_daemon_ipc_invalid_json() {
        let core = HostedCore::init(vec![]).unwrap();
        let reader = std::io::Cursor::new("this-is-not-json\n");
        let mut writer = Vec::new();

        let res = run_daemon(&core, reader, &mut writer);
        assert!(res.is_ok());

        let output_str = String::from_utf8(writer).unwrap();
        let resp: serde_json::Value = serde_json::from_str(&output_str).unwrap();
        assert!(resp["error"].as_str().unwrap().contains("Invalid JSON request"));
    }

    #[test]
    fn test_daemon_ipc_execution_error() {
        let core = HostedCore::init(vec![]).unwrap();
        let req = LocalExecutionRequest {
            orb_name: "missing-orb".to_string(),
            input_json: serde_json::json!({}),
            fuel_limit: None,
            memory_limit: None,
            workspace_path: None,
            allowed_domains: None,
            env_vars: None,
        };

        let req_json = serde_json::to_string(&req).unwrap();
        let input_data = format!("{}\n", req_json);
        let reader = std::io::Cursor::new(input_data);
        let mut writer = Vec::new();

        let res = run_daemon(&core, reader, &mut writer);
        assert!(res.is_ok());

        let output_str = String::from_utf8(writer).unwrap();
        let resp: serde_json::Value = serde_json::from_str(&output_str).unwrap();
        assert!(resp["error"].as_str().unwrap().contains("Execution failed"));
    }
}
