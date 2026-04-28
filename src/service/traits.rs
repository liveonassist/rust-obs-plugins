use std::ffi::{CStr, CString};

use crate::output::OutputRef;
use crate::source::traits::CreateError;
use crate::{prelude::DataObj, properties::Properties};

use super::context::{ConnectInfo, CreatableServiceContext, Resolution, ServiceRef};

/// The mandatory trait every service must implement.
///
/// `Serviceable` identifies the service type and constructs its
/// per-instance state. Pair with the optional traits in this module via
/// [`ServiceInfoBuilder`](super::ServiceInfoBuilder) to wire up callbacks
/// for the OBS service lifecycle.
///
/// Service plugins represent streaming destinations such as Twitch,
/// YouTube, or generic RTMP — anything that an output binds to in order
/// to negotiate a protocol, server URL, and credentials.
pub trait Serviceable: Sized {
    /// Returns the globally-unique identifier for this service type.
    ///
    /// OBS records this id in a process-global table; it must be stable
    /// across plugin loads and unique among all registered services.
    fn get_id() -> &'static CStr;

    /// Constructs a new service instance.
    ///
    /// Called by OBS each time a service of this type is created, either
    /// from saved settings or via `obs_service_create`.
    fn create(
        context: &mut CreatableServiceContext<'_>,
        service: ServiceRef,
    ) -> Result<Self, CreateError>;
}

/// Provides a localized, user-visible display name for the service.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_name`](super::ServiceInfoBuilder::enable_get_name).
pub trait GetNameService {
    /// Returns the display name shown in OBS UIs.
    fn get_name() -> &'static CStr;
}

/// Notified when an output bound to this service activates.
///
/// Receives the latest settings so the service can capture credentials
/// or session state at the moment of go-live. Enable with
/// [`ServiceInfoBuilder::enable_activate`](super::ServiceInfoBuilder::enable_activate).
pub trait ActivateService: Sized {
    /// Called by OBS when an output is about to start streaming to the
    /// service.
    fn activate(&mut self, settings: &mut DataObj);
}

/// Notified when an output bound to this service deactivates.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_deactivate`](super::ServiceInfoBuilder::enable_deactivate).
pub trait DeactivateService: Sized {
    /// Called by OBS when streaming to the service has stopped.
    fn deactivate(&mut self);
}

/// Applies a new settings object to a service instance.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_update`](super::ServiceInfoBuilder::enable_update).
pub trait UpdateService: Sized {
    /// Applies updated settings.
    fn update(&mut self, settings: &mut DataObj);
}

/// Populates the default settings written into a freshly-created service.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_defaults`](super::ServiceInfoBuilder::enable_get_defaults).
pub trait GetDefaultsService {
    /// Writes default values into `settings`.
    fn get_defaults(settings: &mut DataObj);
}

/// Builds the user-facing [`Properties`] panel for the service.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_properties`](super::ServiceInfoBuilder::enable_get_properties).
pub trait GetPropertiesService: Sized {
    /// Returns the property tree OBS will render in the service settings
    /// dialog.
    fn get_properties(&self) -> Properties;
}

/// Last-chance hook invoked just before an output bound to this service
/// is started, with the encoders not yet initialized.
///
/// Returning `false` aborts the start. Enable with
/// [`ServiceInfoBuilder::enable_initialize`](super::ServiceInfoBuilder::enable_initialize).
pub trait InitializeService: Sized {
    /// Returns `true` to allow startup, `false` to abort.
    fn initialize(&mut self, output: &mut OutputRef) -> bool;
}

/// Reports the ingest URL the service is currently configured to push to.
///
/// Returning `None` reports a null URL to OBS. The returned [`CStr`]
/// must remain valid until the next call to this method on the same
/// instance, which is satisfied by storing the string inside `self`.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_url`](super::ServiceInfoBuilder::enable_get_url).
pub trait GetUrlService: Sized {
    /// Returns the configured ingest URL.
    fn get_url(&self) -> Option<&CStr>;
}

/// Reports the stream key the service is currently configured with.
///
/// Returning `None` reports a null key to OBS. The returned [`CStr`]
/// must remain valid until the next call.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_key`](super::ServiceInfoBuilder::enable_get_key).
pub trait GetKeyService: Sized {
    /// Returns the configured stream key.
    fn get_key(&self) -> Option<&CStr>;
}

/// Reports the username the service is currently configured with.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_username`](super::ServiceInfoBuilder::enable_get_username).
pub trait GetUsernameService: Sized {
    /// Returns the configured username.
    fn get_username(&self) -> Option<&CStr>;
}

/// Reports the password the service is currently configured with.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_password`](super::ServiceInfoBuilder::enable_get_password).
pub trait GetPasswordService: Sized {
    /// Returns the configured password.
    fn get_password(&self) -> Option<&CStr>;
}

/// Modifies the encoder settings OBS plans to use with this service.
///
/// Useful for clamping bitrates, forcing keyframe intervals, or
/// otherwise reshaping encoder configuration to match server-side
/// constraints. Enable with
/// [`ServiceInfoBuilder::enable_apply_encoder_settings`](super::ServiceInfoBuilder::enable_apply_encoder_settings).
pub trait ApplyEncoderSettingsService: Sized {
    /// Mutates the encoder settings before they are applied. Either
    /// argument may be `None` if the corresponding encoder is absent.
    fn apply_encoder_settings(
        &mut self,
        video_encoder_settings: Option<&mut DataObj>,
        audio_encoder_settings: Option<&mut DataObj>,
    );
}

/// Returns the OBS output type that should be paired with this service
/// (e.g. `c"rtmp_output"`).
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_output_type`](super::ServiceInfoBuilder::enable_get_output_type).
pub trait GetOutputTypeService: Sized {
    /// Returns the registered id of the preferred output.
    fn get_output_type(&self) -> Option<&CStr>;
}

/// Reports the resolutions the service supports.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_supported_resolutions`](super::ServiceInfoBuilder::enable_get_supported_resolutions).
pub trait GetSupportedResolutionsService: Sized {
    /// Returns the list of supported resolutions.
    fn get_supported_resolutions(&self) -> Vec<Resolution>;
}

/// Reports the maximum frame rate the service accepts.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_max_fps`](super::ServiceInfoBuilder::enable_get_max_fps).
pub trait GetMaxFpsService: Sized {
    /// Returns the maximum FPS, in whole frames per second.
    fn get_max_fps(&self) -> i32;
}

/// Reports the maximum bitrates the service accepts, in kilobits per
/// second.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_max_bitrate`](super::ServiceInfoBuilder::enable_get_max_bitrate).
pub trait GetMaxBitrateService: Sized {
    /// Returns `(video_kbps, audio_kbps)`.
    fn get_max_bitrate(&self) -> (i32, i32);
}

/// Reports the video codecs the service ingest accepts.
///
/// Each [`CString`] should be a libobs codec id (e.g. `"h264"`,
/// `"hevc"`, `"av1"`). The returned list is rebuilt and stored on the
/// service wrapper for the duration of the call. Enable with
/// [`ServiceInfoBuilder::enable_get_supported_video_codecs`](super::ServiceInfoBuilder::enable_get_supported_video_codecs).
pub trait GetSupportedVideoCodecsService: Sized {
    /// Returns the list of supported video codec ids.
    fn get_supported_video_codecs(&self) -> Vec<CString>;
}

/// Reports the audio codecs the service ingest accepts.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_supported_audio_codecs`](super::ServiceInfoBuilder::enable_get_supported_audio_codecs).
pub trait GetSupportedAudioCodecsService: Sized {
    /// Returns the list of supported audio codec ids.
    fn get_supported_audio_codecs(&self) -> Vec<CString>;
}

/// Returns the streaming protocol identifier this service negotiates
/// (e.g. `c"RTMP"`, `c"RTMPS"`, `c"WHIP"`).
///
/// Enable with
/// [`ServiceInfoBuilder::enable_get_protocol`](super::ServiceInfoBuilder::enable_get_protocol).
pub trait GetProtocolService: Sized {
    /// Returns the protocol identifier.
    fn get_protocol(&self) -> Option<&CStr>;
}

/// Returns one piece of connect-time information identified by [`ConnectInfo`].
///
/// This is the v31+ replacement for the per-field `get_url` / `get_key`
/// / etc. callbacks; OBS prefers it when both are implemented. Enable with
/// [`ServiceInfoBuilder::enable_get_connect_info`](super::ServiceInfoBuilder::enable_get_connect_info).
pub trait GetConnectInfoService: Sized {
    /// Returns the requested field, or `None` to report a null value.
    fn get_connect_info(&self, kind: ConnectInfo) -> Option<&CStr>;
}

/// Reports whether the service is currently in a state that permits a
/// connection attempt.
///
/// Enable with
/// [`ServiceInfoBuilder::enable_can_try_to_connect`](super::ServiceInfoBuilder::enable_can_try_to_connect).
pub trait CanTryToConnectService: Sized {
    /// Returns `true` if the service has enough configuration to
    /// attempt a connection.
    fn can_try_to_connect(&self) -> bool;
}
