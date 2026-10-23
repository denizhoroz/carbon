//! Write Carbon extensions in Rust.
//!
//! An extension's code is a WebAssembly component: a `cdylib` crate built
//! with `cargo build --release --target wasm32-wasip2`, named by `wasm` in
//! its `extension.toml`. Implement [`Extension`] and register it:
//!
//! ```ignore
//! use carbon_extension_api::{Extension, notify, register_extension};
//!
//! struct Hello;
//!
//! impl Extension for Hello {
//!     fn init() -> Result<(), String> { Ok(()) }
//!     fn run_command(id: String) -> Result<(), String> {
//!         notify(&format!("ran {id}"));
//!         Ok(())
//!     }
//!     fn language_server_command(_: String) -> Result<Option<ServerCommand>, String> { Ok(None) }
//!     fn render_view(_: String) -> Result<String, String> { Ok("[]".into()) }
//! }
//!
//! register_extension!(Hello);
//! ```
//!
//! Calls that reach outside the extension ([`read_file`], [`run_process`],
//! [`http_get`]) need a capability in the manifest that the user granted;
//! without it they return an error.

#[doc(hidden)]
pub mod bindings {
    // In a module: at the crate root the exported macro clashes with its own
    // re-export (wit-bindgen 0.57).
    wit_bindgen::generate!({
        path: "../carbon-core/wit/since_v0.1.0",
        world: "extension",
        pub_export_macro: true,
        default_bindings_module: "carbon_extension_api::bindings",
    });
}

/// What an extension implements: one function per export of the world.
pub use bindings::Guest as Extension;
pub use bindings::carbon::extension::host::*;

/// Registers the type that implements [`Extension`].
#[macro_export]
macro_rules! register_extension {
    ($extension:ident) => {
        $crate::bindings::export!($extension with_types_in $crate::bindings);
    };
}
