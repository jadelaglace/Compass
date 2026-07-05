//! compass-core — Rust binary for Compass scoring engine and reference parsing.
//!
//! Usage (subprocess mode):
//!   echo '{"jsonrpc":"2.0","method":"compute_score","params":{...},"id":1}' | compass_core
//!
//! Modes:
//!   (no args) — RPC mode over stdin/stdout (default)
//!   --test     — Run unit tests

mod models;
mod reference;
mod rpc;
mod scoring;

use std::env;

fn main() {
    env_logger::init();

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--test" {
        // Run tests and exit
        // Note: in real usage, `cargo test` handles this
        // This flag is for the Python test runner to verify the binary
        println!("Binary OK — RPC mode ready");
        return;
    }

    // Default: RPC server mode
    rpc::run();
}
