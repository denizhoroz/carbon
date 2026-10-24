fn main() {
    // The kit builds syntax highlighters and parses on the main thread, and
    // extension grammars run as WebAssembly on the calling thread's stack;
    // Windows gives the main thread 1 MB, which a WASM grammar overflowed
    // (2026-10-23). Zed raises it to 8 MB the same way.
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target == "windows" && env == "msvc" {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}
