// Edition 2024 promotes calls to unsafe fns inside `unsafe fn` bodies into a
// warn-by-default lint. Most of this crate predates that change; gate it off
// crate-wide so we don't blanket-rewrite every callsite. New code should still
// prefer explicit `unsafe { … }` blocks.
#![allow(unsafe_op_in_unsafe_fn)]

//! # `obs-rs`
//!
//! Safe Rust bindings for authoring [OBS Studio] plugins. The crate
//! wraps libobs's `obs_*_info` registration tables in idiomatic Rust APIs
//! so plugins can register sources, filters, transitions, encoders, and
//! outputs without writing any unsafe FFI plumbing themselves.
//!
//! [OBS Studio]: https://obsproject.com
//!
//! ## Cargo configuration
//!
//! Plugins are dynamic libraries loaded by OBS at runtime. Add `obs-rs` as
//! a dependency and configure the crate as a `cdylib`, substituting
//! `<module-name>` with the name of your plugin:
//!
//! ```toml
//! [dependencies]
//! obs-rs = "0.1"
//!
//! [lib]
//! name = "<module-name>"
//! crate-type = ["cdylib"]
//! ```
//!
//! ## Authoring a plugin
//!
//! 1. Define a type for the plugin's per-module state and implement
//!    [`module::Module`] on it.
//! 2. Define one or more types holding the per-instance state of each
//!    source, encoder, or output your plugin provides, and implement the
//!    matching `*able` trait
//!    ([`source::Sourceable`], [`encoder::Encodable`], or
//!    [`output::Outputable`]).
//! 3. Implement the optional callback traits you need (display name,
//!    update, render, encode, …).
//! 4. From [`Module::load`](crate::module::Module::load), use the supplied
//!    [`LoadContext`](crate::module::LoadContext) to build the
//!    registrations and hand them to OBS.
//! 5. Wire everything up at the crate root with [`obs_register_module!`].
//!
//! ~~~
//! use std::ffi::CStr;
//! use obs_rs::{
//!     // Everything required for modules
//!     prelude::*,
//!     // Everything required for creating a source
//!     source::*,
//!     // Macro for registering modules
//!     obs_register_module,
//! };
//!
//! // The module that will handle creating the source.
//! struct TestModule {
//!     context: ModuleRef
//! };
//!
//! // The source that will be shown inside OBS.
//! struct TestSource;
//!
//! // Implement the Sourceable trait for TestSource, this is required for each
//! // source.
//! // It allows you to specify the source ID and type.
//! impl Sourceable for TestSource {
//!     fn get_id() -> &'static CStr {
//!         c"test_source"
//!     }
//!
//!     fn get_type() -> SourceType {
//!         SourceType::Filter
//!     }
//!
//!     fn create(
//!         create: &mut CreatableSourceContext<Self>,
//!         _source: SourceRef
//!     ) -> Result<Self, CreateError> {
//!         Ok(Self)
//!     }
//! }
//!
//! // Allow OBS to show a name for the source
//! impl GetNameSource for TestSource {
//!     fn get_name() -> &'static CStr {
//!         c"Test Source"
//!     }
//! }
//!
//! // Implement the Module trait for TestModule. This will handle the creation
//! // of the source and has some methods for telling OBS a bit about itself.
//! impl Module for TestModule {
//!     fn new(context: ModuleRef) -> Self {
//!         Self { context }
//!     }
//!
//!     fn get_ctx(&self) -> &ModuleRef {
//!         &self.context
//!     }
//!
//!     // Load the module - create all sources, returning true if all went
//!     // well.
//!     fn load(&mut self, load_context: &mut LoadContext) -> bool {
//!         // Create the source
//!         let source = load_context
//!             .create_source_builder::<TestSource>()
//!             // Since GetNameSource is implemented, this method needs to be
//!             // called to enable it.
//!             .enable_get_name()
//!             .build();
//!
//!         // Tell OBS about the source so that it will show it.
//!         load_context.register_source(source);
//!
//!         // Nothing could have gone wrong, so return true.
//!         true
//!     }
//!
//!     fn description() -> &'static CStr {
//!         c"A great test module."
//!     }
//!
//!     fn name() -> &'static CStr {
//!         c"Test Module"
//!     }
//!
//!     fn author() -> &'static CStr {
//!         c"Bennett"
//!     }
//! }
//! ~~~
//!
//! ## Installing
//!
//! 1. Build with `cargo build --release`.
//! 2. Copy the resulting shared library into OBS's plugins directory
//!    (e.g. `/usr/lib/obs-plugins/` on Linux,
//!    `~/Library/Application Support/obs-studio/plugins/<name>/bin/` on
//!    macOS, or `%ProgramFiles%\obs-studio\obs-plugins\64bit\` on
//!    Windows).
//! 3. Restart OBS. The plugin will appear in the relevant pickers.

/// Raw, unsafe bindings to the OBS C API. Re-exported so plugins can drop
/// down to FFI when needed.
pub use obs_rs_sys;

/// Helpers for wrapping reference-counted OBS pointer types.
#[macro_use]
pub mod wrapper;
/// Bindings for `obs_data_t` — OBS's JSON-shaped settings store.
pub mod data;
/// Bindings for authoring custom OBS encoders.
pub mod encoder;
/// Bindings to OBS Studio's frontend API.
///
/// The frontend API exposes the UI-side controls of OBS Studio:
/// streaming, recording, scenes, transitions, profiles, save/load
/// callbacks, and so on. It is only available when the plugin runs
/// inside the OBS Studio application; it is not part of `libobs` and is
/// gated behind the `frontend-api` feature.
#[cfg(feature = "frontend-api")]
pub mod frontend;
/// Graphics and rendering primitives (effects, textures, samplers).
pub mod graphics;
mod hotkey;
/// Bridge between the [`log`] crate and the OBS logging subsystem.
pub mod log;
/// Audio and video format definitions, plus borrowed views over OBS
/// audio/video buffers.
pub mod media;
/// Module entrypoint, registration, and lifecycle hooks.
pub mod module;
/// Bindings for authoring custom OBS outputs.
pub mod output;
/// Builders for the `obs_properties_t` panels OBS renders in plugin
/// settings dialogs.
pub mod properties;
/// Crate-wide [`Error`] and [`Result`] types.
pub mod result;
/// Bindings for authoring custom OBS services.
pub mod service;
/// Bindings for authoring custom OBS sources.
pub mod source;
/// String helpers for interop with OBS C strings.
pub mod string;

mod native_enum;

/// Re-exports of the types most plugins reach for.
///
/// Glob-importing `obs_rs::prelude::*` brings the [`Module`](module::Module)
/// trait, the source-side context types, [`DataObj`](data::DataObj) and
/// friends, and the C-string helpers into scope.
pub mod prelude {
    pub use crate::data::{DataArray, DataObj, FromDataItem};
    pub use crate::module::*;
    pub use crate::source::context::*;
    pub use crate::string::*;
}

pub use result::{Error, Result};
