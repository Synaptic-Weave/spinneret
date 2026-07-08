use anyhow::Result;
use spinneret::HostedCore;
use std::env;
use std::io;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mode = args.iter()
        .position(|arg| arg == "--mode")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("daemon");

    // Initialize search directory for Orbs. 
    // Default to a folder under user's home directory or the current executable path
    let mut search_paths = vec![];
    
    if let Ok(home) = env::var("HOME") {
        search_paths.push(PathBuf::from(home).join(".spinneret").join("orbs"));
    }
    
    // Add current working directory and a standard 'orbs' folder for local dev
    search_paths.push(PathBuf::from("."));
    search_paths.push(PathBuf::from("./orbs"));
    search_paths.push(PathBuf::from("../orbs"));
    search_paths.push(PathBuf::from("../../orbs"));

    // If an explicit orbs path was passed via --orbs-dir, prioritize it
    if let Some(idx) = args.iter().position(|arg| arg == "--orbs-dir") {
        if let Some(dir) = args.get(idx + 1) {
            search_paths.insert(0, PathBuf::from(dir));
        }
    }

    let core = HostedCore::init(search_paths)?;

    match mode {
        "daemon" => {
            // Secure Local IPC mode: reads JSON lines from stdin, outputs JSON lines to stdout
            let stdin = io::stdin();
            let stdout = io::stdout();
            let reader = stdin.lock();
            spinneret::run_daemon(&core, reader, stdout)?;
        }
        "server" => {
            // This is reserved for Phase 3 (gRPC/HTTP daemon).
            println!("Spinneret HTTP/gRPC local daemon mode is not fully implemented yet.");
            println!("Please use --mode daemon for secure stdin/stdout JSON-RPC instead.");
        }
        _ => {
            eprintln!("Unknown mode: {}. Use --mode daemon or --mode server.", mode);
            std::process::exit(1);
        }
    }

    Ok(())
}
