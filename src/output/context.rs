use std::ffi::{CStr, CString};

use obs_rs_sys::{
    obs_encoder_t, obs_enum_output_types, obs_enum_outputs, obs_output_active, obs_output_audio,
    obs_output_begin_data_capture, obs_output_can_begin_data_capture, obs_output_can_pause,
    obs_output_create, obs_output_end_data_capture, obs_output_force_stop,
    obs_output_get_audio_encoder, obs_output_get_delay, obs_output_get_frames_dropped,
    obs_output_get_id, obs_output_get_name, obs_output_get_ref, obs_output_get_service,
    obs_output_get_total_bytes, obs_output_get_total_frames, obs_output_get_video_encoder,
    obs_output_initialize_encoders, obs_output_pause, obs_output_paused, obs_output_release,
    obs_output_set_audio_encoder, obs_output_set_delay, obs_output_set_media,
    obs_output_set_service, obs_output_set_video_encoder, obs_output_start, obs_output_stop,
    obs_output_t, obs_output_video,
};

use crate::hotkey::HotkeyCallbacks;
use crate::media::{audio::AudioRef, video::VideoRef};
use crate::service::context::ServiceRef;
use crate::string::cstring_from_ptr;
use crate::{Error, Result};
use crate::{hotkey::Hotkey, prelude::DataObj, wrapper::PtrWrapper};

#[deprecated = "use `OutputRef` instead"]
pub type OutputContext = OutputRef;

/// A reference-counted handle to a live OBS output instance.
///
/// `OutputRef` is the safe Rust counterpart to libobs's `obs_output_t`.
/// Cloning increments the underlying reference count; dropping releases it.
/// The handle exposes lifecycle controls (start, stop, pause), encoder
/// bindings, and statistics reporting on the underlying output.
///
/// See the [OBS reference][docs] for the underlying C API.
///
/// [docs]: https://obsproject.com/docs/reference-outputs.html#c.obs_output_t
pub struct OutputRef {
    pub(crate) inner: *mut obs_output_t,
}

impl_ptr_wrapper!(
    @ptr: inner,
    OutputRef,
    obs_output_t,
    obs_output_get_ref,
    obs_output_release
);

unsafe extern "C" fn enum_proc(params: *mut std::ffi::c_void, output: *mut obs_output_t) -> bool {
    let mut v = unsafe { Box::<Vec<*mut obs_output_t>>::from_raw(params as *mut _) };
    v.push(output);
    // Hand the box back to the caller as a raw pointer; OBS still owns it.
    let _ = Box::into_raw(v);
    true
}

impl OutputRef {
    /// Creates a new output instance of the given type.
    ///
    /// `id` selects the output type to instantiate (matching an
    /// [`Outputable::get_id`](super::traits::Outputable::get_id)),
    /// `name` is the user-visible instance name, and `settings` are the
    /// initial settings passed to the output's `create` callback.
    pub fn new(id: &CStr, name: &CStr, settings: Option<DataObj<'_>>) -> Result<Self> {
        let settings = match settings {
            Some(data) => unsafe { data.as_ptr_mut() },
            None => std::ptr::null_mut(),
        };
        let output = unsafe {
            obs_output_create(id.as_ptr(), name.as_ptr(), settings, std::ptr::null_mut())
        };

        unsafe { Self::from_raw_unchecked(output) }.ok_or(Error::NulPointer("obs_output_cretae"))
    }

    /// Returns handles to every output OBS currently knows about.
    pub fn all_outputs() -> Vec<Self> {
        let outputs = Vec::<*mut obs_output_t>::new();
        let params = Box::into_raw(Box::new(outputs));
        unsafe {
            // `obs_enum_outputs` would return `weak_ref`, so `get_ref` needed
            obs_enum_outputs(Some(enum_proc), params as *mut _);
        }
        let outputs = unsafe { Box::from_raw(params) };
        outputs
            .into_iter()
            .filter_map(OutputRef::from_raw)
            .collect()
    }
    /// Returns the registered identifier of every output type known to
    /// OBS.
    pub fn all_types() -> Vec<String> {
        let mut types = Vec::new();
        let mut id = std::ptr::null();
        for idx in 0.. {
            unsafe {
                if !obs_enum_output_types(idx, &mut id) {
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

    /// Returns the registered identifier of this output's type.
    pub fn output_id(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_output_get_id(self.inner)) }
            .ok_or(Error::NulPointer("obs_output_get_id"))
    }

    /// Returns the user-visible name of the output instance.
    pub fn name(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_output_get_name(self.inner)) }
            .ok_or(Error::NulPointer("obs_output_get_name"))
    }

    /// Starts the output. Returns `true` if delivery began.
    pub fn start(&mut self) -> bool {
        unsafe { obs_output_start(self.inner) }
    }

    /// Stops the output gracefully.
    pub fn stop(&mut self) {
        unsafe { obs_output_stop(self.inner) }
    }

    /// Stops the output immediately, without flushing pending data.
    pub fn force_stop(&mut self) {
        unsafe { obs_output_force_stop(self.inner) }
    }

    /// Returns whether the output is currently delivering data.
    pub fn is_active(&self) -> bool {
        unsafe { obs_output_active(self.inner) }
    }

    /// Configures a delivery delay, in seconds. `flags` accepts the
    /// `OBS_OUTPUT_DELAY_*` constants.
    pub fn set_delay(&mut self, delay_secs: u32, flags: u32) {
        unsafe { obs_output_set_delay(self.inner, delay_secs, flags) }
    }

    /// Returns the configured delivery delay, in seconds.
    pub fn delay(&self) -> u32 {
        unsafe { obs_output_get_delay(self.inner) }
    }

    /// Returns whether the output supports pausing.
    pub fn can_pause(&self) -> bool {
        unsafe { obs_output_can_pause(self.inner) }
    }

    /// Pauses or resumes the output. Returns `true` on success.
    pub fn pause(&mut self, pause: bool) -> bool {
        unsafe { obs_output_pause(self.inner, pause) }
    }

    /// Returns whether the output is currently paused.
    pub fn is_paused(&self) -> bool {
        unsafe { obs_output_paused(self.inner) }
    }

    /// Binds a video encoder to this output.
    ///
    /// # Safety
    ///
    /// `encoder` must be a valid `obs_encoder_t*` for which the caller
    /// retains ownership. The output does not assume ownership of the
    /// pointer.
    pub unsafe fn set_video_encoder(&mut self, encoder: *mut obs_encoder_t) {
        // TODO: later should change *mut obs_encoder_t to something like EncoderContext
        unsafe { obs_output_set_video_encoder(self.inner, encoder) }
    }

    /// Returns the bound video encoder, or null if none is set.
    pub fn video_encoder(&self) -> *mut obs_encoder_t {
        unsafe { obs_output_get_video_encoder(self.inner) }
    }

    /// Binds an audio encoder to track `idx`.
    ///
    /// # Safety
    ///
    /// `encoder` must be a valid `obs_encoder_t*` for which the caller
    /// retains ownership. The output does not assume ownership of the
    /// pointer.
    pub unsafe fn set_audio_encoder(&mut self, encoder: *mut obs_encoder_t, idx: usize) {
        // TODO: later should change *mut obs_encoder_t to something like EncoderContext
        unsafe { obs_output_set_audio_encoder(self.inner, encoder, idx as _) }
    }

    /// Returns the audio encoder bound to track `idx`, or null if none is
    /// set.
    pub fn audio_encoder(&self, idx: usize) -> *mut obs_encoder_t {
        unsafe { obs_output_get_audio_encoder(self.inner, idx as _) }
    }

    /// Initializes the bound encoders. Returns `true` on success.
    pub fn init_encoders(&mut self, flags: u32) -> bool {
        unsafe { obs_output_initialize_encoders(self.inner, flags) }
    }

    /// Returns whether the output is ready to begin capturing data.
    pub fn can_start_capture(&self, flags: u32) -> bool {
        unsafe { obs_output_can_begin_data_capture(self.inner, flags) }
    }

    /// Begins data capture, returning `true` on success.
    pub fn start_capture(&mut self, flags: u32) -> bool {
        unsafe { obs_output_begin_data_capture(self.inner, flags) }
    }

    /// Ends the current data-capture session.
    pub fn stop_capture(&mut self) {
        unsafe { obs_output_end_data_capture(self.inner) }
    }

    /// Returns the video output bound to this output.
    pub fn video(&self) -> VideoRef {
        let video = unsafe { obs_output_video(self.inner) };
        VideoRef::from_raw(video)
    }

    /// Returns the audio output bound to this output.
    pub fn audio(&self) -> AudioRef {
        let audio = unsafe { obs_output_audio(self.inner) };
        AudioRef::from_raw(audio)
    }

    /// Equivalent to [`set_video_and_audio`](Self::set_video_and_audio).
    pub fn set_media(&mut self, video: VideoRef, audio: AudioRef) {
        self.set_video_and_audio(video, audio)
    }

    /// Binds the given video and audio outputs to this output.
    pub fn set_video_and_audio(&mut self, video: VideoRef, audio: AudioRef) {
        unsafe { obs_output_set_media(self.inner, video.pointer, audio.pointer) }
    }

    /// Returns the total number of bytes the output has delivered.
    pub fn total_bytes(&self) -> u64 {
        unsafe { obs_output_get_total_bytes(self.inner) }
    }

    /// Returns the number of frames the output has dropped.
    pub fn frames_dropped(&self) -> u32 {
        unsafe { obs_output_get_frames_dropped(self.inner) as u32 }
    }

    /// Returns the number of frames the output has delivered.
    pub fn total_frames(&self) -> u32 {
        unsafe { obs_output_get_total_frames(self.inner) as u32 }
    }

    /// Binds a service to this output. Streaming outputs read the
    /// protocol, ingest URL, and credentials they need from the service.
    ///
    /// libobs takes its own reference; the supplied [`ServiceRef`] keeps
    /// its reference too, so the caller can drop it without invalidating
    /// the binding.
    pub fn set_service(&mut self, service: &ServiceRef) {
        unsafe { obs_output_set_service(self.inner, service.inner) }
    }

    /// Returns the service currently bound to the output, or `None` if
    /// none has been set.
    pub fn service(&self) -> Option<ServiceRef> {
        let ptr = unsafe { obs_output_get_service(self.inner) };
        ServiceRef::from_raw(ptr)
    }
}

/// Construction-time context handed to [`Outputable::create`].
///
/// Carries the initial [`DataObj`] settings supplied by OBS and lets the
/// output register hotkeys that fire callbacks with mutable access to the
/// output's per-instance state.
///
/// [`Outputable::create`]: super::traits::Outputable::create
pub struct CreatableOutputContext<'a, D> {
    pub(crate) hotkey_callbacks: HotkeyCallbacks<D>,
    /// Initial output settings.
    pub settings: DataObj<'a>,
}

impl<'a, D> CreatableOutputContext<'a, D> {
    /// Constructs a new context wrapping the given settings object.
    pub fn from_raw(settings: DataObj<'a>) -> Self {
        Self {
            hotkey_callbacks: vec![],
            settings,
        }
    }

    /// Registers a hotkey that invokes `func` while the output instance is
    /// alive. `name` is the persistent identifier and `description` is the
    /// localized label shown in the hotkey configuration UI.
    pub fn register_hotkey<F: FnMut(&mut Hotkey, &mut D) + 'static>(
        &mut self,
        name: impl Into<CString>,
        description: impl Into<CString>,
        func: F,
    ) {
        self.hotkey_callbacks
            .push((name.into(), description.into(), Box::new(func)));
    }
}
