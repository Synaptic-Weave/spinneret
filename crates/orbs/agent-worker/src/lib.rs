// Agent Worker Orb
// This orb is responsible for the core LLM loop.
// Note: It does NOT have network access capabilities and must compose with http-client to reach the outside world.

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    println!("Agent Worker initialized. Awaiting composed tools...");
}
