use std::ffi::CStr;

use obs_sys_rs::{obs_key_event, obs_mouse_event};

use super::context::{CreatableSourceContext, GlobalContext, VideoRenderContext};
use super::{EnumActiveContext, EnumAllContext, SourceRef, SourceType};
use crate::data::DataObj;
use crate::media::state::MediaState;
use crate::media::{audio::AudioDataContext, video::VideoDataSourceContext};
use crate::properties::Properties;

/// The error type returned from fallible plugin construction callbacks.
///
/// Any `Display + Send + Sync` error type converts into `CreateError` via
/// the standard `Into` impl, so callers can return `anyhow::Error`,
/// `&'static str`, or a custom enum without wrapping.
pub type CreateError = Box<dyn std::error::Error + Send + Sync>;

/// The mandatory trait every source must implement.
///
/// `Sourceable` identifies the source, classifies it, and constructs the
/// per-instance state. Implementations are paired with the optional traits
/// in this module by [`SourceInfoBuilder`].
///
/// [`SourceInfoBuilder`]: super::SourceInfoBuilder
pub trait Sourceable: Sized {
    /// Returns the globally-unique identifier for this source type.
    ///
    /// OBS records this id in a process-global table; it must be stable
    /// across plugin loads and unique among all registered sources.
    fn get_id() -> &'static CStr;

    /// Returns the role this source plays in the OBS pipeline.
    fn get_type() -> SourceType;

    /// Constructs a new source instance.
    ///
    /// Called by OBS each time a new source of this type is created.
    fn create(
        create: &mut CreatableSourceContext<Self>,
        source: SourceRef,
    ) -> Result<Self, CreateError>;
}

/// Provides a localized, user-visible display name for the source.
///
/// Enable with
/// [`SourceInfoBuilder::enable_get_name`](super::SourceInfoBuilder::enable_get_name).
pub trait GetNameSource {
    /// Returns the display name shown in OBS UIs.
    fn get_name() -> &'static CStr;
}

/// Reports the render width of the source, in pixels.
///
/// Enable with
/// [`SourceInfoBuilder::enable_get_width`](super::SourceInfoBuilder::enable_get_width).
pub trait GetWidthSource: Sized {
    /// Returns the source's current render width.
    fn get_width(&self) -> u32;
}

/// Reports the render height of the source, in pixels.
///
/// Enable with
/// [`SourceInfoBuilder::enable_get_height`](super::SourceInfoBuilder::enable_get_height).
pub trait GetHeightSource: Sized {
    /// Returns the source's current render height.
    fn get_height(&self) -> u32;
}

/// Notifies the source that it has become active in the program output.
pub trait ActivateSource: Sized {
    /// Called when the source begins rendering to the program output.
    fn activate(&mut self);
}

/// Notifies the source that it is no longer active in the program output.
pub trait DeactivateSource: Sized {
    /// Called when the source stops rendering to the program output.
    fn deactivate(&mut self);
}

/// Applies a new settings object to a running source.
///
/// Enable with
/// [`SourceInfoBuilder::enable_update`](super::SourceInfoBuilder::enable_update).
pub trait UpdateSource: Sized {
    /// Applies updated settings.
    fn update(&mut self, settings: &mut DataObj, context: &mut GlobalContext);
}

/// Receives mouse-wheel events from OBS.
///
/// Enable with
/// [`SourceInfoBuilder::enable_mouse_wheel`](super::SourceInfoBuilder::enable_mouse_wheel).
pub trait MouseWheelSource: Sized {
    /// Called when the user scrolls the mouse wheel over the source.
    fn mouse_wheel(&mut self, event: obs_mouse_event, xdelta: i32, ydelta: i32);
}

/// Receives mouse-button click events from OBS.
///
/// Enable with
/// [`SourceInfoBuilder::enable_mouse_click`](super::SourceInfoBuilder::enable_mouse_click).
pub trait MouseClickSource: Sized {
    /// Called when the user presses or releases a mouse button.
    fn mouse_click(
        &mut self,
        event: obs_mouse_event,
        button: super::MouseButton,
        pressed: bool,
        click_count: u8,
    );
}

/// Receives mouse-movement events from OBS.
///
/// Enable with
/// [`SourceInfoBuilder::enable_mouse_move`](super::SourceInfoBuilder::enable_mouse_move).
pub trait MouseMoveSource: Sized {
    /// Called as the cursor moves over the source. `leave` is `true` when
    /// the cursor leaves the source's bounds.
    fn mouse_move(&mut self, event: obs_mouse_event, leave: bool);
}

/// Receives keyboard events from OBS.
///
/// Enable with
/// [`SourceInfoBuilder::enable_key_click`](super::SourceInfoBuilder::enable_key_click).
pub trait KeyClickSource: Sized {
    /// Called for each key press or release while the source has focus.
    fn key_click(&mut self, event: obs_key_event, pressed: bool);
}

/// Receives focus-change notifications from OBS.
///
/// Enable with
/// [`SourceInfoBuilder::enable_focus`](super::SourceInfoBuilder::enable_focus).
pub trait FocusSource: Sized {
    /// Called when the source gains or loses keyboard focus.
    fn focus(&mut self, focused: bool);
}

/// Renders the source to the active OBS render target.
///
/// Enable with
/// [`SourceInfoBuilder::enable_video_render`](super::SourceInfoBuilder::enable_video_render).
pub trait VideoRenderSource: Sized {
    /// Called once per video frame for synchronous rendering.
    fn video_render(&mut self, context: &mut GlobalContext, render: &mut VideoRenderContext);
}

/// Renders the source's audio.
///
/// Enable with
/// [`SourceInfoBuilder::enable_audio_render`](super::SourceInfoBuilder::enable_audio_render).
pub trait AudioRenderSource: Sized {
    /// Called when OBS asks the source to produce audio output.
    fn audio_render(&mut self, context: &mut GlobalContext);
}

/// Builds the user-facing [`Properties`] panel for the source.
///
/// Enable with
/// [`SourceInfoBuilder::enable_get_properties`](super::SourceInfoBuilder::enable_get_properties).
pub trait GetPropertiesSource: Sized {
    /// Returns the property tree that OBS will render in the source's
    /// settings UI.
    fn get_properties(&self) -> Properties;
}

/// Receives a per-frame tick callback regardless of whether the source is
/// being rendered.
///
/// Enable with
/// [`SourceInfoBuilder::enable_video_tick`](super::SourceInfoBuilder::enable_video_tick).
pub trait VideoTickSource: Sized {
    /// Called once per frame with the elapsed seconds since the last tick.
    fn video_tick(&mut self, seconds: f32);
}

/// Enumerates the sources currently active beneath this source (for
/// scenes, transitions, etc.).
pub trait EnumActiveSource: Sized {
    /// Called by OBS to enumerate active child sources.
    fn enum_active_sources(&mut self, context: &EnumActiveContext);
}

/// Enumerates every source beneath this source, including inactive ones.
pub trait EnumAllSource: Sized {
    /// Called by OBS to enumerate all child sources.
    fn enum_all_sources(&mut self, context: &EnumAllContext);
}

/// Notifies a transition source that a transition has begun.
pub trait TransitionStartSource: Sized {
    /// Called when a transition starts.
    fn transition_start(&mut self);
}

/// Notifies a transition source that a transition has completed.
pub trait TransitionStopSource: Sized {
    /// Called when a transition stops.
    fn transition_stop(&mut self);
}

/// Audio-filter callback. Mutates an upstream audio buffer in place.
///
/// Enable with
/// [`SourceInfoBuilder::enable_filter_audio`](super::SourceInfoBuilder::enable_filter_audio).
pub trait FilterAudioSource: Sized {
    /// Called for each audio buffer flowing through the filter chain.
    fn filter_audio(&mut self, audio: &mut AudioDataContext);
}

/// Video-filter callback. Mutates an upstream video frame in place (for
/// CPU-side filters that don't render through the graphics pipeline).
///
/// Enable with
/// [`SourceInfoBuilder::enable_filter_video`](super::SourceInfoBuilder::enable_filter_video).
pub trait FilterVideoSource: Sized {
    /// Called for each video frame flowing through the filter chain.
    fn filter_video(&mut self, video: &mut VideoDataSourceContext);
}

/// Handles play/pause requests on a media source.
pub trait MediaPlayPauseSource: Sized {
    /// Called to pause or resume playback.
    fn play_pause(&mut self, pause: bool);
}

/// Reports the playback state of a media source.
pub trait MediaGetStateSource: Sized {
    /// Returns the current [`MediaState`].
    fn get_state(&self) -> MediaState;
}

/// Handles seek requests on a media source.
pub trait MediaSetTimeSource: Sized {
    /// Seeks to `milliseconds` into the current media.
    fn set_time(&mut self, milliseconds: i64);
}

/// Populates the default settings written into a freshly-created source.
pub trait GetDefaultsSource {
    /// Writes default values into `settings`.
    fn get_defaults(settings: &mut DataObj);
}

/// Restarts media playback from the beginning.
pub trait MediaRestartSource: Sized {
    /// Called to restart playback.
    fn restart(&mut self);
}

/// Stops media playback.
pub trait MediaStopSource: Sized {
    /// Called to stop playback.
    fn stop(&mut self);
}

/// Advances to the next item in a media playlist.
pub trait MediaNextSource: Sized {
    /// Called to advance to the next item.
    fn next(&mut self);
}

/// Returns to the previous item in a media playlist.
pub trait MediaPreviousSource: Sized {
    /// Called to return to the previous item.
    fn previous(&mut self);
}

/// Reports the total duration of the current media, in milliseconds.
pub trait MediaGetDurationSource: Sized {
    /// Returns the total duration of the current media.
    fn get_duration(&self) -> i64;
}

/// Reports the current playback position, in milliseconds.
pub trait MediaGetTimeSource: Sized {
    /// Returns the current playback position.
    fn get_time(&self) -> i64;
}
