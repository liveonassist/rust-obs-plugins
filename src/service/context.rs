use std::ffi::{CStr, CString};

use obs_sys_rs::{
    obs_data_t, obs_enum_service_types, obs_enum_services, obs_get_service_by_name,
    obs_service_apply_encoder_settings, obs_service_can_try_to_connect, obs_service_connect_info,
    obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_BEARER_TOKEN,
    obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_ENCRYPT_PASSPHRASE,
    obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_PASSWORD,
    obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_SERVER_URL,
    obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_STREAM_KEY,
    obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_USERNAME, obs_service_create,
    obs_service_create_private, obs_service_get_connect_info, obs_service_get_display_name,
    obs_service_get_id, obs_service_get_max_bitrate, obs_service_get_max_fps, obs_service_get_name,
    obs_service_get_protocol, obs_service_get_ref, obs_service_get_settings,
    obs_service_get_supported_resolutions, obs_service_get_type, obs_service_release,
    obs_service_resolution, obs_service_t, obs_service_update,
};

use crate::data::DataObj;
use crate::string::cstring_from_ptr;
use crate::wrapper::PtrWrapper;
use crate::{Error, Result};

/// A 2D resolution exposed to libobs as `obs_service_resolution`.
///
/// Used by [`GetSupportedResolutionsService`](super::traits::GetSupportedResolutionsService)
/// to advertise the resolutions a service ingest accepts.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Resolution {
    /// Width in pixels.
    pub width: i32,
    /// Height in pixels.
    pub height: i32,
}

impl Resolution {
    /// Constructs a resolution.
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }

    pub(crate) fn into_raw(self) -> obs_service_resolution {
        obs_service_resolution {
            cx: self.width,
            cy: self.height,
        }
    }
}

/// One of the connect-info fields requested by libobs through
///
/// Mirrors [`obs_service_connect_info`]. Unknown values are preserved
/// through the [`Other`](Self::Other) variant so newer libobs revisions
/// that introduce additional fields don't break existing services.
///
/// # Platform-specific raw representation
///
/// libobs declares `obs_service_connect_info` as an unnamed C enum.
/// bindgen picks the underlying Rust type from whatever the C compiler
/// would have used:
///
/// * On Unix-like targets, GCC/Clang give unsigned-only enums an
///   unsigned underlying type, so bindgen emits `c_uint` (i.e. [`u32`]).
/// * On Windows MSVC, every unnamed enum is signed `int` regardless of
///   its values, so bindgen emits `c_int` (i.e. [`i32`]).
///
/// To keep the FFI surface honest, [`as_raw`](Self::as_raw) and
/// [`from_raw`](Self::from_raw) speak in the bindgen-emitted
/// [`obs_service_connect_info`] type alias rather than picking a fixed
/// Rust integer. That alias resolves to [`u32`] off Windows and [`i32`]
/// on Windows, so the value can be passed to
/// [`obs_service_get_connect_info`] (and stored in the
/// [`obs_service_info::get_connect_info`] field) without any `as` cast at
/// the call site.
///
/// [`GetConnectInfoService`]: super::traits::GetConnectInfoService
/// [`obs_service_info::get_connect_info`]: obs_sys_rs::obs_service_info::get_connect_info
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ConnectInfo {
    /// The ingest URL the output should connect to.
    ServerUrl,
    /// The stream key (alias of `STREAM_ID` in libobs).
    StreamKey,
    /// The user name credential.
    Username,
    /// The password credential.
    Password,
    /// The passphrase used to encrypt the stream (e.g. SRT encryption).
    EncryptPassphrase,
    /// A bearer token credential (e.g. WHIP).
    BearerToken,
    /// A connect-info field this version of `obs-rs` does not name.
    Other(u32),
}

impl ConnectInfo {
    /// Maps the libobs raw kind into a [`ConnectInfo`].
    // `as u32` in the catch-all arm is a no-op on Linux (where the
    // bindgen typedef is `c_uint`) but mandatory on Windows MSVC (where
    // it's `c_int`); silence clippy on the platform that doesn't need it.
    #[allow(non_upper_case_globals, non_snake_case, clippy::unnecessary_cast)]
    pub fn from_raw(kind: obs_service_connect_info) -> Self {
        match kind {
            obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_SERVER_URL => Self::ServerUrl,
            obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_STREAM_KEY => Self::StreamKey,
            obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_USERNAME => Self::Username,
            obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_PASSWORD => Self::Password,
            obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_ENCRYPT_PASSPHRASE => {
                Self::EncryptPassphrase
            }
            obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_BEARER_TOKEN => Self::BearerToken,
            other => Self::Other(other as u32),
        }
    }

    /// Returns the underlying libobs raw kind.
    #[allow(non_upper_case_globals, non_snake_case, clippy::unnecessary_cast)]
    pub fn as_raw(&self) -> obs_service_connect_info {
        match self {
            Self::ServerUrl => obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_SERVER_URL,
            Self::StreamKey => obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_STREAM_KEY,
            Self::Username => obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_USERNAME,
            Self::Password => obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_PASSWORD,
            Self::EncryptPassphrase => {
                obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_ENCRYPT_PASSPHRASE
            }
            Self::BearerToken => obs_service_connect_info_OBS_SERVICE_CONNECT_INFO_BEARER_TOKEN,
            Self::Other(v) => *v as obs_service_connect_info,
        }
    }
}

/// A reference-counted handle to a live OBS service instance.
///
/// `ServiceRef` is the safe Rust counterpart to libobs's `obs_service_t`.
/// Cloning increments the underlying reference count; dropping releases it.
/// Pass a `ServiceRef` to
/// [`OutputRef::set_service`](crate::output::OutputRef::set_service) to
/// bind it to a streaming output.
///
/// See the [OBS reference][docs] for the underlying C API.
///
/// [docs]: https://obsproject.com/docs/reference-services.html
pub struct ServiceRef {
    pub(crate) inner: *mut obs_service_t,
}

impl_ptr_wrapper!(
    @ptr: inner,
    ServiceRef,
    obs_service_t,
    obs_service_get_ref,
    obs_service_release
);

unsafe extern "C" fn enum_proc(params: *mut std::ffi::c_void, service: *mut obs_service_t) -> bool {
    let mut v = unsafe { Box::<Vec<*mut obs_service_t>>::from_raw(params as *mut _) };
    v.push(service);
    let _ = Box::into_raw(v);
    true
}

impl ServiceRef {
    /// Creates a new service instance of the given type.
    ///
    /// `id` selects the service type (matching a
    /// [`Serviceable::get_id`](super::traits::Serviceable::get_id)),
    /// `name` is the user-visible instance name, and `settings` are the
    /// initial settings handed to the service's `create` callback.
    pub fn new(id: &CStr, name: &CStr, settings: Option<DataObj<'_>>) -> Result<Self> {
        let settings = match settings {
            Some(data) => unsafe { data.as_ptr_mut() },
            None => std::ptr::null_mut(),
        };
        let service = unsafe {
            obs_service_create(id.as_ptr(), name.as_ptr(), settings, std::ptr::null_mut())
        };
        unsafe { Self::from_raw_unchecked(service) }.ok_or(Error::NulPointer("obs_service_create"))
    }

    /// Creates a private (unnamed, not enumerable) service instance.
    pub fn new_private(id: &CStr, name: &CStr, settings: Option<DataObj<'_>>) -> Result<Self> {
        let settings = match settings {
            Some(data) => unsafe { data.as_ptr_mut() },
            None => std::ptr::null_mut(),
        };
        let service = unsafe { obs_service_create_private(id.as_ptr(), name.as_ptr(), settings) };
        unsafe { Self::from_raw_unchecked(service) }
            .ok_or(Error::NulPointer("obs_service_create_private"))
    }

    /// Looks up a service by its user-visible name.
    pub fn by_name(name: &CStr) -> Option<Self> {
        let ptr = unsafe { obs_get_service_by_name(name.as_ptr()) };
        unsafe { Self::from_raw_unchecked(ptr) }
    }

    /// Returns handles to every service OBS currently knows about.
    pub fn all_services() -> Vec<Self> {
        let services: Vec<*mut obs_service_t> = Vec::new();
        let params = Box::into_raw(Box::new(services));
        unsafe {
            obs_enum_services(Some(enum_proc), params as *mut _);
        }
        let services = unsafe { Box::from_raw(params) };
        services
            .into_iter()
            .filter_map(ServiceRef::from_raw)
            .collect()
    }

    /// Returns the registered identifier of every service type known to
    /// OBS.
    pub fn all_types() -> Vec<String> {
        let mut types = Vec::new();
        let mut id: *const std::os::raw::c_char = std::ptr::null();
        for idx in 0.. {
            unsafe {
                if !obs_enum_service_types(idx, &mut id) {
                    break;
                }
            }
            if id.is_null() {
                types.push(String::new())
            } else {
                types.push(unsafe { CStr::from_ptr(id) }.to_string_lossy().into_owned())
            }
        }
        types
    }

    /// Returns the localized display name registered for `id`, if known.
    pub fn display_name_for(id: &CStr) -> Option<CString> {
        unsafe { cstring_from_ptr(obs_service_get_display_name(id.as_ptr())) }
    }

    /// Returns the registered identifier of this service's type.
    pub fn service_id(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_service_get_id(self.inner)) }
            .ok_or(Error::NulPointer("obs_service_get_id"))
    }

    /// Returns the user-visible name of the service instance.
    pub fn name(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_service_get_name(self.inner)) }
            .ok_or(Error::NulPointer("obs_service_get_name"))
    }

    /// Returns the libobs internal `type` string for the service. In
    /// practice this matches [`service_id`](Self::service_id).
    pub fn type_id(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_service_get_type(self.inner)) }
            .ok_or(Error::NulPointer("obs_service_get_type"))
    }

    /// Returns the protocol identifier reported by the service
    /// (e.g. `RTMP`, `RTMPS`, `WHIP`).
    pub fn protocol(&self) -> Option<CString> {
        unsafe { cstring_from_ptr(obs_service_get_protocol(self.inner)) }
    }

    /// Returns the current settings of the service. The wrapper does not
    /// take an additional reference; cloning the [`DataObj`] borrow if
    /// needed is the caller's responsibility.
    pub fn settings(&self) -> Option<DataObj<'_>> {
        let raw = unsafe { obs_service_get_settings(self.inner) };
        unsafe { DataObj::from_raw_unchecked(raw) }
    }

    /// Updates the service's settings.
    pub fn update(&mut self, settings: &mut DataObj) {
        let ptr = (unsafe { settings.as_ptr_mut() }) as *mut obs_data_t;
        unsafe { obs_service_update(self.inner, ptr) }
    }

    /// Returns the field requested by `kind`, or `None` if the service
    /// reports a null value.
    pub fn connect_info(&self, kind: ConnectInfo) -> Option<CString> {
        unsafe { cstring_from_ptr(obs_service_get_connect_info(self.inner, kind.as_raw())) }
    }

    /// Returns whether the service is in a state that permits a
    /// connection attempt.
    pub fn can_try_to_connect(&self) -> bool {
        unsafe { obs_service_can_try_to_connect(self.inner) }
    }

    /// Returns the maximum frame rate the service accepts, in whole
    /// frames per second.
    pub fn max_fps(&self) -> i32 {
        let mut fps: std::os::raw::c_int = 0;
        unsafe { obs_service_get_max_fps(self.inner, &mut fps) };
        fps
    }

    /// Returns `(video_kbps, audio_kbps)`.
    pub fn max_bitrate(&self) -> (i32, i32) {
        let mut video: std::os::raw::c_int = 0;
        let mut audio: std::os::raw::c_int = 0;
        unsafe { obs_service_get_max_bitrate(self.inner, &mut video, &mut audio) };
        (video, audio)
    }

    /// Returns the resolutions the service advertises as supported.
    pub fn supported_resolutions(&self) -> Vec<Resolution> {
        let mut ptr: *mut obs_service_resolution = std::ptr::null_mut();
        let mut count: usize = 0;
        unsafe { obs_service_get_supported_resolutions(self.inner, &mut ptr, &mut count) };
        if ptr.is_null() || count == 0 {
            return Vec::new();
        }
        let slice = unsafe { std::slice::from_raw_parts(ptr, count) };
        slice.iter().map(|r| Resolution::new(r.cx, r.cy)).collect()
    }

    /// Applies any service-specific tweaks to the given encoder settings.
    /// Either argument may be `None`.
    pub fn apply_encoder_settings(
        &mut self,
        video_settings: Option<&mut DataObj>,
        audio_settings: Option<&mut DataObj>,
    ) {
        let video = match video_settings {
            Some(s) => (unsafe { s.as_ptr_mut() }) as *mut obs_data_t,
            None => std::ptr::null_mut(),
        };
        let audio = match audio_settings {
            Some(s) => (unsafe { s.as_ptr_mut() }) as *mut obs_data_t,
            None => std::ptr::null_mut(),
        };
        unsafe { obs_service_apply_encoder_settings(self.inner, video, audio) }
    }
}

/// Construction-time context handed to [`Serviceable::create`].
///
/// Carries the initial [`DataObj`] settings supplied by OBS. Unlike the
/// source/output contexts, services do not register hotkeys.
///
/// [`Serviceable::create`]: super::traits::Serviceable::create
pub struct CreatableServiceContext<'a> {
    /// Initial service settings.
    pub settings: DataObj<'a>,
}

impl<'a> CreatableServiceContext<'a> {
    /// Constructs a new context wrapping the given settings object.
    pub fn from_raw(settings: DataObj<'a>) -> Self {
        Self { settings }
    }
}
