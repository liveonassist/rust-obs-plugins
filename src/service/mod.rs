//! Bindings for authoring custom OBS services.
//!
//! A service represents a streaming destination — Twitch, YouTube,
//! X (formerly Twitter), Kick, a custom RTMP/SRT/WHIP ingest, and so on.
//! Outputs bind to a service to learn the protocol, ingest URL, and
//! credentials to use, and to give the service a chance to reshape
//! encoder settings before the stream starts.
//!
//! # Authoring a service
//!
//! 1. Define a type that holds the per-instance state of your service.
//! 2. Implement [`Serviceable`](crate::service::traits::Serviceable) for
//!    it. This is mandatory and provides the service identifier plus the
//!    `create` hook.
//! 3. Implement any of the optional traits
//!    ([`GetNameService`](crate::service::traits::GetNameService),
//!    [`UpdateService`](crate::service::traits::UpdateService),
//!    [`GetPropertiesService`](crate::service::traits::GetPropertiesService),
//!    [`GetConnectInfoService`](crate::service::traits::GetConnectInfoService),
//!    [`GetProtocolService`](crate::service::traits::GetProtocolService),
//!    …) that match the OBS callbacks your service needs.
//! 4. Build a [`ServiceInfo`](crate::service::ServiceInfo) with
//!    [`ServiceInfoBuilder`](crate::service::ServiceInfoBuilder), opting
//!    each optional trait in with the matching `enable_*` method, and
//!    pass it to [`LoadContext::register_service`].
//!
//! Services exposing v31+ ingests should prefer
//! [`GetConnectInfoService`](crate::service::traits::GetConnectInfoService)
//! over the per-field `get_url` / `get_key` / `get_username` /
//! `get_password` callbacks. libobs prefers `get_connect_info` when it is
//! present and falls back to the per-field callbacks otherwise.
//!
//! # Example
//!
//! ```ignore
//! use std::ffi::{CStr, CString};
//! use obs_rs::{
//!     prelude::*,
//!     service::*,
//!     properties::Properties,
//!     obs_register_module,
//! };
//!
//! struct MyTwitch {
//!     url: CString,
//!     key: CString,
//! }
//!
//! impl Serviceable for MyTwitch {
//!     fn get_id() -> &'static CStr { c"my_twitch" }
//!
//!     fn create(
//!         ctx: &mut CreatableServiceContext<'_>,
//!         _service: ServiceRef,
//!     ) -> Result<Self, CreateError> {
//!         let url = ctx
//!             .settings
//!             .get::<CString>(c"server")
//!             .unwrap_or_else(|| c"rtmp://live.twitch.tv/app".to_owned());
//!         let key = ctx
//!             .settings
//!             .get::<CString>(c"key")
//!             .unwrap_or_default();
//!         Ok(Self { url, key })
//!     }
//! }
//!
//! impl GetNameService for MyTwitch {
//!     fn get_name() -> &'static CStr { c"My Twitch" }
//! }
//!
//! impl GetConnectInfoService for MyTwitch {
//!     fn get_connect_info(&self, kind: ConnectInfo) -> Option<&CStr> {
//!         match kind {
//!             ConnectInfo::ServerUrl => Some(self.url.as_c_str()),
//!             ConnectInfo::StreamKey => Some(self.key.as_c_str()),
//!             _ => None,
//!         }
//!     }
//! }
//!
//! impl GetProtocolService for MyTwitch {
//!     fn get_protocol(&self) -> Option<&CStr> { Some(c"RTMP") }
//! }
//!
//! impl GetOutputTypeService for MyTwitch {
//!     fn get_output_type(&self) -> Option<&CStr> { Some(c"rtmp_output") }
//! }
//!
//! // In `Module::load`:
//! // load_context.register_service(
//! //     load_context
//! //         .create_service_builder::<MyTwitch>()
//! //         .enable_get_name()
//! //         .enable_get_connect_info()
//! //         .enable_get_protocol()
//! //         .enable_get_output_type()
//! //         .build(),
//! // );
//! ```
//!
//! [`LoadContext::register_service`]: crate::module::LoadContext::register_service

use paste::item;
use std::marker::PhantomData;

use obs_sys_rs::obs_service_info;

pub mod context;
mod ffi;
pub mod traits;

pub use context::*;
pub use traits::*;

/// A fully-configured service registration, ready to be handed to OBS.
///
/// Produced by [`ServiceInfoBuilder::build`] and consumed by
/// [`LoadContext::register_service`], which keeps the underlying
/// allocation alive until the module is unloaded.
///
/// [`LoadContext::register_service`]: crate::module::LoadContext::register_service
pub struct ServiceInfo {
    info: Box<obs_service_info>,
}

impl ServiceInfo {
    /// Consumes the wrapper and returns the raw `obs_service_info`
    /// pointer.
    ///
    /// # Safety
    ///
    /// Transfers ownership of the heap allocation to the caller, which
    /// must later reclaim it via `Box::from_raw`. In normal use this is
    /// performed by [`LoadContext`](crate::module::LoadContext) at module
    /// unload.
    pub unsafe fn into_raw(self) -> *mut obs_service_info {
        Box::into_raw(self.info)
    }
}

impl AsRef<obs_service_info> for ServiceInfo {
    fn as_ref(&self) -> &obs_service_info {
        self.info.as_ref()
    }
}

/// Builder that wires up the OBS callbacks for a custom service.
///
/// Obtain a builder from
/// [`LoadContext::create_service_builder`](crate::module::LoadContext::create_service_builder)
/// and call the matching `enable_*` method for each optional trait you
/// implemented on `D`. Each `enable_*` method is bounded on the
/// corresponding trait, so the compiler will refuse to enable a callback
/// the service cannot service. Finalize with [`build`](Self::build).
///
/// # Examples
///
/// ```ignore
/// let service = load_context
///     .create_service_builder::<MyTwitch>()
///     .enable_get_name()
///     .enable_update()
///     .enable_get_properties()
///     .enable_get_connect_info()
///     .enable_get_protocol()
///     .enable_get_output_type()
///     .build();
/// ```
pub struct ServiceInfoBuilder<D: Serviceable> {
    __data: PhantomData<D>,
    info: obs_service_info,
}

impl<D: Serviceable> ServiceInfoBuilder<D> {
    pub(crate) fn new() -> Self {
        Self {
            __data: PhantomData,
            info: obs_service_info {
                id: D::get_id().as_ptr(),
                create: Some(ffi::create::<D>),
                destroy: Some(ffi::destroy::<D>),
                type_data: std::ptr::null_mut(),
                ..Default::default()
            },
        }
    }

    /// Finalizes the builder into a [`ServiceInfo`] suitable for
    /// [`LoadContext::register_service`].
    ///
    /// [`LoadContext::register_service`]: crate::module::LoadContext::register_service
    pub fn build(self) -> ServiceInfo {
        ServiceInfo {
            info: Box::new(self.info),
        }
    }
}

macro_rules! impl_service_builder {
    ($($f:ident => $t:ident)*) => ($(
        item! {
            impl<D: Serviceable + [<$t>]> ServiceInfoBuilder<D> {
                pub fn [<enable_$f>](mut self) -> Self {
                    self.info.[<$f>] = Some(ffi::[<$f>]::<D>);
                    self
                }
            }
        }
    )*)
}

impl_service_builder! {
    get_name => GetNameService
    activate => ActivateService
    deactivate => DeactivateService
    update => UpdateService
    get_defaults => GetDefaultsService
    get_properties => GetPropertiesService
    initialize => InitializeService
    get_url => GetUrlService
    get_key => GetKeyService
    get_username => GetUsernameService
    get_password => GetPasswordService
    apply_encoder_settings => ApplyEncoderSettingsService
    get_output_type => GetOutputTypeService
    get_supported_resolutions => GetSupportedResolutionsService
    get_max_fps => GetMaxFpsService
    get_max_bitrate => GetMaxBitrateService
    get_supported_video_codecs => GetSupportedVideoCodecsService
    get_protocol => GetProtocolService
    get_supported_audio_codecs => GetSupportedAudioCodecsService
    get_connect_info => GetConnectInfoService
    can_try_to_connect => CanTryToConnectService
}
