fn main() {
    let test_mode = std::env::var("TEST_MODE").unwrap_or_else(|_| "http".to_string());
    println!("hello-wasm running in mode: {}", test_mode);

    match test_mode.as_str() {
        "fuel" => {
            // Infinite loop to exhaust CPU fuel
            let mut i = 0u64;
            loop {
                i = i.wrapping_add(1);
                // Print occasionally to avoid rustc optimization or optimize-out, or use a volatile-like operation
                if i % 1_000_000 == 0 {
                    println!("fuel burner loop: {}", i);
                }
            }
        }
        "memory" => {
            // Allocate memory in chunks to trigger the memory limiter trap
            println!("Attempting to allocate memory...");
            let mut chunks = Vec::new();
            for i in 0..1000 {
                // Allocate 1MB chunks
                let chunk = vec![0u8; 1024 * 1024];
                chunks.push(chunk);
                println!("Allocated {} MB", i + 1);
            }
        }
        "env" => {
            let key = "MY_VAR";
            let expected = "MY_VALUE";
            let val = std::env::var(key).expect("MY_VAR environment variable not set");
            if val != expected {
                panic!("Expected MY_VAR to be '{}', got '{}'", expected, val);
            }
            println!("Environment verification successful!");
        }
        "workspace" => {
            let path = std::path::Path::new("/worktree/test.txt");
            println!("Writing to workspace at /worktree/test.txt");
            std::fs::write(path, "Spinneret Workspace Test").expect("Failed to write to workspace file");
            
            println!("Reading back from workspace...");
            let content = std::fs::read_to_string(path).expect("Failed to read from workspace file");
            if content != "Spinneret Workspace Test" {
                panic!("Content mismatch: '{}'", content);
            }
            println!("Workspace verification successful!");
        }
        _ => {
            // Default HTTP test behavior
            let host = std::env::var("TARGET_HOST").unwrap_or_else(|_| "example.com".to_string());
            println!("Requesting: {}", host);
            
            let headers = wasi::http::types::Headers::new();
            let req = wasi::http::types::OutgoingRequest::new(headers);
            req.set_method(&wasi::http::types::Method::Get).unwrap();
            req.set_scheme(Some(&wasi::http::types::Scheme::Https)).unwrap();
            req.set_authority(Some(&host)).unwrap();
            req.set_path_with_query(Some("/")).unwrap();
            
            match wasi::http::outgoing_handler::handle(req, None) {
                Ok(_) => println!("Success!"),
                Err(_) => println!("Error!"),
            }
        }
    }
}

