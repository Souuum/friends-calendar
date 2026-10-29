// `sqlx::migrate!` embeds backend/migrations at compile time, but on stable
// Rust it cannot tell cargo to watch that directory. Without this, adding a
// migration leaves an incremental build (and `cargo test`) running the old
// embedded set until some .rs file happens to change.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
