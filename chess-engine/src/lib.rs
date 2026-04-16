pub mod engine;
pub mod eval;
pub mod search;
pub mod uci;

pub fn current_version_tag() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut parts = version.split('.');
    let major = parts.next().unwrap_or("0");
    let minor = parts.next().unwrap_or("0");
    format!("v{}.{}", major, minor)
}

pub fn uci_engine_name() -> String {
    format!("Wiggum Engine {}", current_version_tag())
}
