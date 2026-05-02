//! Build script — tells cargo to re-link the crate whenever the env vars
//! that get baked into the binary via `option_env!` change.
//!
//! Without this, cargo would happily reuse a cached object file with a
//! stale (or missing) API key.

fn main() {
    println!("cargo:rerun-if-env-changed=RESEND_API_KEY");
    println!("cargo:rerun-if-env-changed=RESEND_FROM");
}
