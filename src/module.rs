//! Module entrypoint and registration plumbing.
//!
//! Every plugin defines a single [`Module`](crate::module::Module) implementation and wires it up
//! with the [`obs_register_module!`](crate::obs_register_module) macro. OBS calls into the module on
//! load to discover the plugin's identity and to let it register the
//! sources, outputs, and encoders it provides.

use crate::encoder::{EncoderInfo, EncoderInfoBuilder, traits::Encodable};
use crate::output::{OutputInfo, OutputInfoBuilder, traits::Outputable};
use crate::service::{ServiceInfo, ServiceInfoBuilder, traits::Serviceable};
use crate::source::{SourceInfo, SourceInfoBuilder, traits::Sourceable};
use crate::string::cstring_from_ptr;
use crate::{Error, Result};
use obs_rs_sys::{
    obs_encoder_info, obs_get_module_author, obs_get_module_description, obs_get_module_file_name,
    obs_get_module_name, obs_module_t, obs_output_info, obs_register_encoder_s,
    obs_register_output_s, obs_register_service_s, obs_register_source_s, obs_service_info,
    obs_source_info, size_t,
};
use std::ffi::{CStr, CString};
use std::marker::PhantomData;

/// Registration scratchpad available during [`Module::load`].
///
/// `LoadContext` exposes builders for sources, outputs, and encoders, and
/// hands the resulting registrations to OBS via the corresponding
/// `register_*` methods. The context owns every registration it accepts
/// for the lifetime of the module; when it is dropped at module unload,
/// the underlying `obs_*_info` allocations are reclaimed.
///
/// Plugins do not construct `LoadContext` directly — one is produced by
/// the runtime glue in [`obs_register_module!`](crate::obs_register_module) and passed into
/// [`Module::load`].
pub struct LoadContext {
    __marker: PhantomData<()>,
    sources: Vec<*mut obs_source_info>,
    outputs: Vec<*mut obs_output_info>,
    encoders: Vec<*mut obs_encoder_info>,
    services: Vec<*mut obs_service_info>,
}

impl LoadContext {
    /// Constructs a fresh `LoadContext`.
    ///
    /// # Safety
    ///
    /// A `LoadContext` is only valid during OBS's module-load phase.
    /// Calling this outside of [`Module::load`] is undefined behaviour at
    /// the C level — the macro-generated entrypoint is the only correct
    /// caller.
    pub unsafe fn new() -> LoadContext {
        LoadContext {
            __marker: PhantomData,
            sources: vec![],
            outputs: vec![],
            encoders: vec![],
            services: vec![],
        }
    }

    /// Returns a fresh [`SourceInfoBuilder`] for the source type `D`.
    pub fn create_source_builder<D: Sourceable>(&self) -> SourceInfoBuilder<D> {
        SourceInfoBuilder::new()
    }

    /// Returns a fresh [`OutputInfoBuilder`] for the output type `D`.
    pub fn create_output_builder<D: Outputable>(&self) -> OutputInfoBuilder<D> {
        OutputInfoBuilder::new()
    }

    /// Returns a fresh [`EncoderInfoBuilder`] for the encoder type `D`.
    pub fn create_encoder_builder<D: Encodable>(&self) -> EncoderInfoBuilder<D> {
        EncoderInfoBuilder::new()
    }

    /// Returns a fresh [`ServiceInfoBuilder`] for the service type `D`.
    pub fn create_service_builder<D: Serviceable>(&self) -> ServiceInfoBuilder<D> {
        ServiceInfoBuilder::new()
    }

    /// Registers a source with OBS.
    ///
    /// The context retains ownership of the underlying allocation until
    /// the module is unloaded.
    pub fn register_source(&mut self, source: SourceInfo) {
        let pointer = source.into_raw();
        unsafe {
            obs_register_source_s(pointer, std::mem::size_of::<obs_source_info>() as size_t);
        };
        self.sources.push(pointer);
    }

    /// Registers an output with OBS.
    ///
    /// The context retains ownership of the underlying allocation until
    /// the module is unloaded.
    pub fn register_output(&mut self, output: OutputInfo) {
        let pointer = unsafe {
            let pointer = output.into_raw();
            obs_register_output_s(pointer, std::mem::size_of::<obs_output_info>() as size_t);
            pointer
        };
        self.outputs.push(pointer);
    }

    /// Registers an encoder with OBS.
    ///
    /// The context retains ownership of the underlying allocation until
    /// the module is unloaded.
    pub fn register_encoder(&mut self, encoder: EncoderInfo) {
        let pointer = unsafe {
            let pointer = encoder.into_raw();
            obs_register_encoder_s(pointer, std::mem::size_of::<obs_encoder_info>() as size_t);
            pointer
        };
        self.encoders.push(pointer);
    }

    /// Registers a service with OBS.
    ///
    /// The context retains ownership of the underlying allocation until
    /// the module is unloaded.
    pub fn register_service(&mut self, service: ServiceInfo) {
        let pointer = unsafe {
            let pointer = service.into_raw();
            obs_register_service_s(pointer, std::mem::size_of::<obs_service_info>() as size_t);
            pointer
        };
        self.services.push(pointer);
    }
}

impl Drop for LoadContext {
    fn drop(&mut self) {
        unsafe {
            for pointer in self.sources.drain(..) {
                drop(Box::from_raw(pointer))
            }
            for pointer in self.outputs.drain(..) {
                drop(Box::from_raw(pointer))
            }
            for pointer in self.encoders.drain(..) {
                drop(Box::from_raw(pointer))
            }
            for pointer in self.services.drain(..) {
                drop(Box::from_raw(pointer))
            }
        }
    }
}

/// The trait every plugin implements.
///
/// `Module` provides OBS with the plugin's identity (name, description,
/// author) and the load/unload lifecycle hooks. A plugin defines exactly
/// one implementation and registers it with [`obs_register_module!`](crate::obs_register_module).
pub trait Module {
    /// Constructs a new module instance bound to the given module
    /// reference. Called by OBS during plugin discovery.
    fn new(ctx: ModuleRef) -> Self;

    /// Returns the module's [`ModuleRef`]. Used by the runtime to satisfy
    /// `obs_current_module`.
    fn get_ctx(&self) -> &ModuleRef;

    /// Registers sources, outputs, and encoders with the supplied
    /// [`LoadContext`]. Returns `false` to abort plugin loading.
    ///
    /// The default implementation registers nothing and returns `true`.
    fn load(&mut self, _load_context: &mut LoadContext) -> bool {
        true
    }

    /// Releases any resources the module holds outside of registrations
    /// owned by the [`LoadContext`].
    ///
    /// The default implementation does nothing.
    fn unload(&mut self) {}

    /// Called once after every plugin in the process has finished
    /// [`load`](Self::load). Use this for cross-module wiring.
    ///
    /// The default implementation does nothing.
    fn post_load(&mut self) {}

    /// Returns the module's user-visible description.
    fn description() -> &'static CStr;

    /// Returns the module's user-visible name.
    fn name() -> &'static CStr;

    /// Returns the module's author.
    fn author() -> &'static CStr;
}

/// Wires a [`Module`] implementation into the OBS plugin entrypoints.
///
/// Generates the `extern "C"` symbols (`obs_module_load`,
/// `obs_module_unload`, `obs_module_name`, …) that OBS calls when loading
/// a plugin. Invoke it once at the crate root, passing the module type:
///
/// ```ignore
/// obs_register_module!(MyModule);
/// ```
#[macro_export]
macro_rules! obs_register_module {
    ($t:ty) => {
        static mut OBS_MODULE: Option<$t> = None;
        static mut LOAD_CONTEXT: Option<$crate::module::LoadContext> = None;

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_set_pointer(
            raw: *mut $crate::obs_rs_sys::obs_module_t,
        ) {
            OBS_MODULE = ModuleRef::from_raw(raw).ok().map(<$t>::new);
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_current_module() -> *mut $crate::obs_rs_sys::obs_module_t {
            if let Some(module) = &OBS_MODULE {
                module.get_ctx().get_raw()
            } else {
                panic!("Could not get current module!");
            }
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_ver() -> u32 {
            $crate::obs_rs_sys::LIBOBS_API_MAJOR_VER
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_load() -> bool {
            let mut module = OBS_MODULE.as_mut().expect("Could not get current module!");
            let mut context = unsafe { $crate::module::LoadContext::new() };
            let ret = module.load(&mut context);
            LOAD_CONTEXT = Some(context);

            ret
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_unload() {
            let mut module = OBS_MODULE.as_mut().expect("Could not get current module!");
            module.unload();
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_post_load() {
            let mut module = OBS_MODULE.as_mut().expect("Could not get current module!");
            module.post_load();
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_name() -> *const std::os::raw::c_char {
            <$t>::name().as_ptr()
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_description() -> *const std::os::raw::c_char {
            <$t>::description().as_ptr()
        }

        #[allow(missing_safety_doc)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn obs_module_author() -> *const std::os::raw::c_char {
            <$t>::author().as_ptr()
        }
    };
}

#[deprecated = "use `ModuleRef` instead"]
pub type ModuleContext = ModuleRef;

/// A handle to the running plugin module.
///
/// `ModuleRef` is the safe Rust counterpart to libobs's `obs_module_t`.
/// It's created by the runtime glue in [`obs_register_module!`](crate::obs_register_module) and
/// surfaced through [`Module::get_ctx`]; plugins typically only read
/// metadata off of it.
pub struct ModuleRef {
    raw: *mut obs_module_t,
}

impl std::fmt::Debug for ModuleRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleRef")
            .field("name", &self.name())
            .field("description", &self.description())
            .field("author", &self.author())
            .field("file_name", &self.file_name())
            .finish()
    }
}

impl ModuleRef {
    /// Wraps a raw `obs_module_t*`.
    ///
    /// Returns [`Error::NulPointer`] if `raw` is null.
    pub fn from_raw(raw: *mut obs_module_t) -> Result<Self> {
        if raw.is_null() {
            Err(Error::NulPointer("obs_module_t"))
        } else {
            Ok(Self { raw })
        }
    }

    /// Returns the underlying `obs_module_t*`.
    ///
    /// # Safety
    ///
    /// The pointer is owned by OBS. Mutating the contents it points to,
    /// or retaining the pointer past the lifetime of the `ModuleRef`,
    /// is undefined behaviour at the C level.
    pub unsafe fn get_raw(&self) -> *mut obs_module_t {
        self.raw
    }
}

impl ModuleRef {
    /// Returns the module's user-visible name.
    pub fn name(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_get_module_name(self.raw)) }
            .ok_or(Error::NulPointer("obs_get_module_name"))
    }

    /// Returns the module's user-visible description.
    pub fn description(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_get_module_description(self.raw)) }
            .ok_or(Error::NulPointer("obs_get_module_description"))
    }

    /// Returns the module's author string.
    pub fn author(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_get_module_author(self.raw)) }
            .ok_or(Error::NulPointer("obs_get_module_author"))
    }

    /// Returns the file name of the loaded module binary.
    pub fn file_name(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_get_module_file_name(self.raw)) }
            .ok_or(Error::NulPointer("obs_get_module_file_name"))
    }
}
