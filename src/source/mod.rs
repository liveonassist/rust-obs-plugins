//! Bindings for authoring custom OBS sources.
//!
//! A source is the unit OBS uses to model anything that produces video
//! and/or audio: capture devices, media files, browsers, scenes, filters
//! that mutate other sources, transitions between scenes, and so on.
//! Plugins register sources at module load time, after which OBS drives
//! them through the standard libobs source lifecycle.
//!
//! # Authoring a source
//!
//! 1. Define a type that holds the per-instance state of your source.
//! 2. Implement [`Sourceable`](crate::source::traits::Sourceable) for it.
//!    This is mandatory and identifies the source, classifies it (input,
//!    filter, scene, transition), and constructs the per-instance state.
//! 3. Implement any of the optional traits in this module
//!    ([`GetNameSource`](crate::source::traits::GetNameSource),
//!    [`VideoRenderSource`](crate::source::traits::VideoRenderSource),
//!    [`AudioRenderSource`](crate::source::traits::AudioRenderSource),
//!    [`UpdateSource`](crate::source::traits::UpdateSource),
//!    [`MouseClickSource`](crate::source::traits::MouseClickSource), …)
//!    that match the OBS callbacks your source needs.
//! 4. Build a [`SourceInfo`](crate::source::SourceInfo) with
//!    [`SourceInfoBuilder`](crate::source::SourceInfoBuilder), opting each
//!    optional trait in with the matching `enable_*` method, and pass it
//!    to [`LoadContext::register_source`].
//!
//! Asynchronous push-style sources (capture device workers, network
//! receivers) are documented separately in [`push`](crate::source::push).
//!
//! [`LoadContext::register_source`]: crate::module::LoadContext::register_source

use paste::item;

pub mod context;
mod ffi;
pub mod push;
pub mod scene;
pub mod traits;

use crate::{Error, Result, media::state::MediaState, string::cstring_from_ptr};

pub use context::*;
pub use push::*;
pub use traits::*;

use obs_sys_rs::{
    OBS_SOURCE_ASYNC_VIDEO, OBS_SOURCE_AUDIO, OBS_SOURCE_CONTROLLABLE_MEDIA,
    OBS_SOURCE_INTERACTION, OBS_SOURCE_VIDEO, obs_filter_get_target, obs_icon_type,
    obs_icon_type_OBS_ICON_TYPE_AUDIO_INPUT, obs_icon_type_OBS_ICON_TYPE_AUDIO_OUTPUT,
    obs_icon_type_OBS_ICON_TYPE_BROWSER, obs_icon_type_OBS_ICON_TYPE_CAMERA,
    obs_icon_type_OBS_ICON_TYPE_COLOR, obs_icon_type_OBS_ICON_TYPE_CUSTOM,
    obs_icon_type_OBS_ICON_TYPE_DESKTOP_CAPTURE, obs_icon_type_OBS_ICON_TYPE_GAME_CAPTURE,
    obs_icon_type_OBS_ICON_TYPE_IMAGE, obs_icon_type_OBS_ICON_TYPE_MEDIA,
    obs_icon_type_OBS_ICON_TYPE_SLIDESHOW, obs_icon_type_OBS_ICON_TYPE_TEXT,
    obs_icon_type_OBS_ICON_TYPE_UNKNOWN, obs_icon_type_OBS_ICON_TYPE_WINDOW_CAPTURE,
    obs_mouse_button_type, obs_mouse_button_type_MOUSE_LEFT, obs_mouse_button_type_MOUSE_MIDDLE,
    obs_mouse_button_type_MOUSE_RIGHT, obs_source_active, obs_source_enabled,
    obs_source_get_base_height, obs_source_get_base_width, obs_source_get_height,
    obs_source_get_id, obs_source_get_name, obs_source_get_ref, obs_source_get_type,
    obs_source_get_width, obs_source_info, obs_source_media_ended, obs_source_media_get_duration,
    obs_source_media_get_state, obs_source_media_get_time, obs_source_media_next,
    obs_source_media_play_pause, obs_source_media_previous, obs_source_media_restart,
    obs_source_media_set_time, obs_source_media_started, obs_source_media_stop,
    obs_source_process_filter_begin, obs_source_process_filter_end,
    obs_source_process_filter_tech_end, obs_source_release, obs_source_set_enabled,
    obs_source_set_name, obs_source_showing, obs_source_skip_video_filter, obs_source_t,
    obs_source_type, obs_source_type_OBS_SOURCE_TYPE_FILTER, obs_source_type_OBS_SOURCE_TYPE_INPUT,
    obs_source_type_OBS_SOURCE_TYPE_SCENE, obs_source_type_OBS_SOURCE_TYPE_TRANSITION,
    obs_source_update,
};

use super::graphics::{
    GraphicsAllowDirectRendering, GraphicsColorFormat, GraphicsEffect, GraphicsEffectContext,
};
use crate::{data::DataObj, native_enum, wrapper::PtrWrapper};

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
};

native_enum!(MouseButton, obs_mouse_button_type {
    Left => MOUSE_LEFT,
    Middle => MOUSE_MIDDLE,
    Right => MOUSE_RIGHT,
});

native_enum!(Icon, obs_icon_type {
    Unknown => OBS_ICON_TYPE_UNKNOWN,
    Image => OBS_ICON_TYPE_IMAGE,
    Color => OBS_ICON_TYPE_COLOR,
    Slideshow => OBS_ICON_TYPE_SLIDESHOW,
    AudioInput => OBS_ICON_TYPE_AUDIO_INPUT,
    AudioOutput => OBS_ICON_TYPE_AUDIO_OUTPUT,
    DesktopCapture => OBS_ICON_TYPE_DESKTOP_CAPTURE,
    WindowCapture => OBS_ICON_TYPE_WINDOW_CAPTURE,
    GameCapture => OBS_ICON_TYPE_GAME_CAPTURE,
    Camera => OBS_ICON_TYPE_CAMERA,
    Text => OBS_ICON_TYPE_TEXT,
    Media => OBS_ICON_TYPE_MEDIA,
    Browser => OBS_ICON_TYPE_BROWSER,
    Custom => OBS_ICON_TYPE_CUSTOM,
});

native_enum!(
/// The role a source plays in the OBS pipeline.
///
/// Returned by [`Sourceable::get_type`] and surfaced through
/// [`SourceRef`]. See the [OBS reference][docs] for the
/// underlying semantics.
///
/// [docs]: https://obsproject.com/docs/reference-sources.html#c.obs_source_get_type
SourceType, obs_source_type {
    /// A standalone input that produces video and/or audio.
    Input => OBS_SOURCE_TYPE_INPUT,
    /// A scene composed of one or more sources.
    Scene => OBS_SOURCE_TYPE_SCENE,
    /// A filter that transforms another source's output.
    Filter => OBS_SOURCE_TYPE_FILTER,
    /// A transition between scenes.
    Transition => OBS_SOURCE_TYPE_TRANSITION,
});

#[deprecated = "use `SourceRef` instead"]
pub type SourceContext = SourceRef;

/// A reference-counted handle to a live OBS source instance.
///
/// `SourceRef` is the safe Rust counterpart to libobs's `obs_source_t`.
/// Cloning increments the underlying reference count; dropping releases it.
/// The handle exposes inspection of the source's identity and dimensions,
/// media-controllable hooks, filter-chain traversal, and the conversions
/// to push-style handles documented in the [`push`] module.
///
/// See the [OBS reference][docs] for the underlying C API.
///
/// [docs]: https://obsproject.com/docs/reference-sources.html#c.obs_source_t
pub struct SourceRef {
    inner: *mut obs_source_t,
}

impl std::fmt::Debug for SourceRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceRef")
            .field("id", &self.id())
            .field("name", &self.name())
            .field("source_id", &self.source_id())
            .field("width", &self.width())
            .field("height", &self.height())
            .field("showing", &self.showing())
            .field("active", &self.active())
            .field("enabled", &self.enabled())
            .finish()
    }
}

impl_ptr_wrapper!(
    @ptr: inner,
    SourceRef,
    obs_source_t,
    obs_source_get_ref,
    obs_source_release
);

impl SourceRef {
    /// Invokes `func` with the next source down the filter chain.
    ///
    /// Has no effect unless this source is a filter; sources of any other
    /// kind are silently ignored.
    pub fn do_with_target<F: FnOnce(&mut SourceRef)>(&mut self, func: F) {
        unsafe {
            if let Ok(SourceType::Filter) = SourceType::from_raw(obs_source_get_type(self.inner)) {
                // doc says "Does not increment the reference."
                let target = obs_filter_get_target(self.inner);
                if let Some(mut context) = SourceRef::from_raw(target) {
                    func(&mut context);
                }
            }
        }
    }

    /// Returns a process-stable identifier for this source.
    ///
    /// Derived from the underlying pointer; identical for any two
    /// [`SourceRef`]s pointing at the same OBS source.
    pub fn id(&self) -> usize {
        self.inner as usize
    }

    /// Returns the source's pre-scale width, in pixels.
    pub fn get_base_width(&self) -> u32 {
        unsafe { obs_source_get_base_width(self.inner) }
    }

    /// Returns the source's pre-scale height, in pixels.
    pub fn get_base_height(&self) -> u32 {
        unsafe { obs_source_get_base_height(self.inner) }
    }

    /// Returns whether the source is currently visible in any scene.
    pub fn showing(&self) -> bool {
        unsafe { obs_source_showing(self.inner) }
    }

    /// Returns whether the source is currently rendered to the program
    /// output.
    pub fn active(&self) -> bool {
        unsafe { obs_source_active(self.inner) }
    }

    /// Returns whether the source is enabled.
    pub fn enabled(&self) -> bool {
        unsafe { obs_source_enabled(self.inner) }
    }

    /// Enables or disables the source.
    pub fn set_enabled(&mut self, enabled: bool) {
        unsafe { obs_source_set_enabled(self.inner, enabled) }
    }

    /// Returns the registered identifier of this source's type.
    pub fn source_id(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_source_get_id(self.inner)) }
            .ok_or(Error::NulPointer("obs_source_get_id"))
    }

    /// Returns the user-visible name of this source instance.
    pub fn name(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_source_get_name(self.inner)) }
            .ok_or(Error::NulPointer("obs_source_get_name"))
    }

    /// Renames the source instance.
    ///
    /// # Panics
    ///
    /// Panics if `name` contains an interior nul byte.
    pub fn set_name(&mut self, name: &str) {
        let cstr = CString::new(name).unwrap();
        unsafe {
            obs_source_set_name(self.inner, cstr.as_ptr());
        }
    }

    /// Returns the source's render width, in pixels.
    pub fn width(&self) -> u32 {
        unsafe { obs_source_get_width(self.inner) }
    }

    /// Returns the source's render height, in pixels.
    pub fn height(&self) -> u32 {
        unsafe { obs_source_get_height(self.inner) }
    }

    /// Pauses or resumes media playback on this source.
    pub fn media_play_pause(&mut self, pause: bool) {
        unsafe {
            obs_source_media_play_pause(self.inner, pause);
        }
    }

    /// Restarts media playback from the beginning.
    pub fn media_restart(&mut self) {
        unsafe {
            obs_source_media_restart(self.inner);
        }
    }

    /// Stops media playback.
    pub fn media_stop(&mut self) {
        unsafe {
            obs_source_media_stop(self.inner);
        }
    }

    /// Advances to the next item in a media playlist.
    pub fn media_next(&mut self) {
        unsafe {
            obs_source_media_next(self.inner);
        }
    }

    /// Returns to the previous item in a media playlist.
    pub fn media_previous(&mut self) {
        unsafe {
            obs_source_media_previous(self.inner);
        }
    }

    /// Returns the total duration of the current media, in milliseconds.
    pub fn media_duration(&self) -> i64 {
        unsafe { obs_source_media_get_duration(self.inner) }
    }

    /// Returns the current playback position, in milliseconds.
    pub fn media_time(&self) -> i64 {
        unsafe { obs_source_media_get_time(self.inner) }
    }

    /// Seeks to `ms` milliseconds into the current media.
    pub fn media_set_time(&mut self, ms: i64) {
        unsafe { obs_source_media_set_time(self.inner, ms) }
    }

    /// Returns the current media playback state.
    ///
    /// # Panics
    ///
    /// Panics if libobs reports an unrecognized state value.
    pub fn media_state(&self) -> MediaState {
        let ret = unsafe { obs_source_media_get_state(self.inner) };
        MediaState::from_raw(ret).expect("Invalid media state value")
    }

    /// Notifies OBS that media playback has started.
    pub fn media_started(&mut self) {
        unsafe {
            obs_source_media_started(self.inner);
        }
    }

    /// Notifies OBS that media playback has ended.
    pub fn media_ended(&mut self) {
        unsafe {
            obs_source_media_ended(self.inner);
        }
    }

    /// Skips this video filter for the current frame, passing through the
    /// upstream source's video unchanged.
    pub fn skip_video_filter(&mut self) {
        unsafe {
            obs_source_skip_video_filter(self.inner);
        }
    }

    /// Drives the filter render path with the given effect.
    ///
    /// The provided closure runs inside an active graphics context wrapped
    /// by `obs_source_process_filter_begin` / `_end`. Has no effect unless
    /// this source is a filter.
    ///
    /// See the [OBS reference][docs] for the underlying semantics.
    ///
    /// [docs]: https://obsproject.com/docs/reference-sources.html#c.obs_source_process_filter_begin
    pub fn process_filter<F: FnOnce(&mut GraphicsEffectContext, &mut GraphicsEffect)>(
        &mut self,
        _render: &mut VideoRenderContext,
        effect: &mut GraphicsEffect,
        (cx, cy): (u32, u32),
        format: GraphicsColorFormat,
        direct: GraphicsAllowDirectRendering,
        func: F,
    ) {
        unsafe {
            if let Ok(SourceType::Filter) = SourceType::from_raw(obs_source_get_type(self.inner))
                && obs_source_process_filter_begin(self.inner, format.as_raw(), direct.as_raw())
            {
                let mut context = GraphicsEffectContext::new();
                func(&mut context, effect);
                obs_source_process_filter_end(self.inner, effect.as_ptr(), cx, cy);
            }
        }
    }

    /// Like [`process_filter`](Self::process_filter), but selects a named
    /// technique on the effect.
    #[allow(clippy::too_many_arguments)]
    pub fn process_filter_tech<F: FnOnce(&mut GraphicsEffectContext, &mut GraphicsEffect)>(
        &mut self,
        _render: &mut VideoRenderContext,
        effect: &mut GraphicsEffect,
        (cx, cy): (u32, u32),
        format: GraphicsColorFormat,
        direct: GraphicsAllowDirectRendering,
        technique: &CStr,
        func: F,
    ) {
        unsafe {
            if let Ok(SourceType::Filter) = SourceType::from_raw(obs_source_get_type(self.inner))
                && obs_source_process_filter_begin(self.inner, format.as_raw(), direct.as_raw())
            {
                let mut context = GraphicsEffectContext::new();
                func(&mut context, effect);
                obs_source_process_filter_tech_end(
                    self.inner,
                    effect.as_ptr(),
                    cx,
                    cy,
                    technique.as_ptr(),
                );
            }
        }
    }

    /// Applies updated settings to this source.
    pub fn update_source_settings(&mut self, settings: &mut DataObj) {
        unsafe {
            obs_source_update(self.inner, settings.as_ptr_mut());
        }
    }
}

/// Marker context passed to [`EnumActiveSource::enum_active_sources`].
///
/// [`EnumActiveSource::enum_active_sources`]: traits::EnumActiveSource::enum_active_sources
pub struct EnumActiveContext {}

/// Marker context passed to [`EnumAllSource::enum_all_sources`].
///
/// [`EnumAllSource::enum_all_sources`]: traits::EnumAllSource::enum_all_sources
pub struct EnumAllContext {}

/// A fully-configured source registration, ready to be handed to OBS.
///
/// Produced by [`SourceInfoBuilder::build`] and consumed by
/// [`LoadContext::register_source`], which keeps the underlying allocation
/// alive until the module is unloaded.
///
/// [`LoadContext::register_source`]: crate::module::LoadContext::register_source
pub struct SourceInfo {
    info: Box<obs_source_info>,
}

impl SourceInfo {
    /// Consumes the wrapper and returns the raw `obs_source_info` pointer.
    ///
    /// Ownership of the heap allocation is transferred to the caller; in
    /// normal use this is performed by
    /// [`LoadContext`](crate::module::LoadContext) at module unload.
    pub fn into_raw(self) -> *mut obs_source_info {
        Box::into_raw(self.info)
    }

    /// Sets the icon shown in the OBS source-picker UI.
    pub fn set_icon(&mut self, icon: Icon) {
        self.info.icon_type = icon.into();
    }
}

impl AsRef<obs_source_info> for SourceInfo {
    fn as_ref(&self) -> &obs_source_info {
        self.info.as_ref()
    }
}

impl AsMut<obs_source_info> for SourceInfo {
    fn as_mut(&mut self) -> &mut obs_source_info {
        self.info.as_mut()
    }
}

/// Builder that wires up the OBS callbacks for a custom source.
///
/// Obtain a builder from
/// [`LoadContext::create_source_builder`](crate::module::LoadContext::create_source_builder)
/// and call the matching `enable_*` method for each optional trait you
/// implemented on `D`. Each `enable_*` method is bounded on the
/// corresponding trait, so the compiler will refuse to enable a callback
/// the source cannot service. Finalize with [`build`](Self::build).
///
/// # Examples
///
/// ```ignore
/// let source = load_context
///     .create_source_builder::<FocusFilter>()
///     .enable_get_name()
///     .enable_video_render()
///     .build();
/// ```
pub struct SourceInfoBuilder<D: Sourceable> {
    __data: PhantomData<D>,
    info: obs_source_info,
}

impl<D: Sourceable> SourceInfoBuilder<D> {
    pub(crate) fn new() -> Self {
        Self {
            __data: PhantomData,
            info: obs_source_info {
                id: D::get_id().as_ptr(),
                type_: D::get_type().as_raw(),
                create: Some(ffi::create::<D>),
                destroy: Some(ffi::destroy::<D>),
                type_data: std::ptr::null_mut(),
                ..Default::default()
            },
        }
    }

    /// Finalizes the builder into a [`SourceInfo`] suitable for
    /// [`LoadContext::register_source`].
    ///
    /// `output_flags` is computed automatically from the callbacks that
    /// have been enabled (e.g. enabling
    /// [`enable_video_render`](Self::enable_video_render) sets
    /// `OBS_SOURCE_VIDEO`).
    ///
    /// [`LoadContext::register_source`]: crate::module::LoadContext::register_source
    pub fn build(mut self) -> SourceInfo {
        if self.info.video_render.is_some() {
            self.info.output_flags |= OBS_SOURCE_VIDEO;
        }

        if self.info.audio_render.is_some() || self.info.filter_audio.is_some() {
            self.info.output_flags |= OBS_SOURCE_AUDIO;
        }

        if self.info.media_get_state.is_some() || self.info.media_play_pause.is_some() {
            self.info.output_flags |= OBS_SOURCE_CONTROLLABLE_MEDIA;
        }

        if self.info.mouse_click.is_some()
            || self.info.mouse_move.is_some()
            || self.info.mouse_wheel.is_some()
            || self.info.focus.is_some()
            || self.info.key_click.is_some()
        {
            self.info.output_flags |= OBS_SOURCE_INTERACTION;
        }

        SourceInfo {
            info: Box::new(self.info),
        }
    }

    /// Sets the icon shown in the OBS source-picker UI.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.info.icon_type = icon.into();
        self
    }

    /// Mark this source as an asynchronous video input (sets
    /// `OBS_SOURCE_ASYNC_VIDEO`). Required to use
    /// [`SourceRef::as_async_video`] and push frames via
    /// [`AsyncVideoSource::output_video`]. Async sources must NOT also
    /// implement `VideoRenderSource`.
    pub fn enable_async_video(mut self) -> Self {
        self.info.output_flags |= OBS_SOURCE_ASYNC_VIDEO;
        self
    }

    /// Mark this source as an audio input (sets `OBS_SOURCE_AUDIO`). Required
    /// to use [`SourceRef::as_async_audio`] and push samples via
    /// [`AsyncAudioSource::output_audio`].
    pub fn enable_async_audio(mut self) -> Self {
        self.info.output_flags |= OBS_SOURCE_AUDIO;
        self
    }
}

macro_rules! impl_source_builder {
    ($($f:ident => $t:ident)*) => ($(
        item! {
            impl<D: Sourceable + [<$t>]> SourceInfoBuilder<D> {
                pub fn [<enable_$f>](mut self) -> Self {
                    self.info.[<$f>] = Some(ffi::[<$f>]::<D>);
                    self
                }
            }
        }
    )*)
}

impl_source_builder! {
    get_name => GetNameSource
    get_width => GetWidthSource
    get_height => GetHeightSource
    activate => ActivateSource
    deactivate => DeactivateSource
    update => UpdateSource
    video_render => VideoRenderSource
    audio_render => AudioRenderSource
    get_properties => GetPropertiesSource
    enum_active_sources => EnumActiveSource
    enum_all_sources => EnumAllSource
    transition_start => TransitionStartSource
    transition_stop => TransitionStopSource
    video_tick => VideoTickSource
    filter_audio => FilterAudioSource
    filter_video => FilterVideoSource
    get_defaults => GetDefaultsSource
    media_play_pause => MediaPlayPauseSource
    media_restart => MediaRestartSource
    media_stop => MediaStopSource
    media_next => MediaNextSource
    media_previous => MediaPreviousSource
    media_get_duration => MediaGetDurationSource
    media_get_time => MediaGetTimeSource
    media_set_time => MediaSetTimeSource
    media_get_state => MediaGetStateSource
    mouse_wheel => MouseWheelSource
    mouse_click => MouseClickSource
    mouse_move => MouseMoveSource
    key_click => KeyClickSource
    focus => FocusSource
}
