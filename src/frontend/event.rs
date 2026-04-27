//! High-level frontend events and event-callback registration.
//!
//! OBS Studio publishes a stream of [`Event`]s to plugins as the user
//! interacts with the UI: scenes change, recording starts, the theme
//! switches, and so on. This module wraps the C-level
//! `obs_frontend_event_cb` machinery in a Rust closure interface.
//!
//! # Example
//!
//! ```no_run
//! use obs_rs::frontend::{Event, add_event_callback};
//!
//! let _guard = add_event_callback(|event| {
//!     if event == Event::SceneChanged {
//!         eprintln!("scene changed!");
//!     }
//! });
//! // _guard is dropped at end of scope, removing the callback.
//! ```
//!
//! # Threading
//!
//! Callbacks run on OBS's UI thread. The closure type is
//! `FnMut(Event) + 'static` — captures must be `Send`-safe only if you
//! intend to share state with another thread.

use std::os::raw::c_void;

#[allow(unused_imports)]
use obs_rs_sys::{
    obs_frontend_add_event_callback, obs_frontend_event,
    obs_frontend_event_OBS_FRONTEND_EVENT_EXIT,
    obs_frontend_event_OBS_FRONTEND_EVENT_FINISHED_LOADING,
    obs_frontend_event_OBS_FRONTEND_EVENT_PREVIEW_SCENE_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_PROFILE_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_PROFILE_CHANGING,
    obs_frontend_event_OBS_FRONTEND_EVENT_PROFILE_LIST_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_PROFILE_RENAMED,
    obs_frontend_event_OBS_FRONTEND_EVENT_RECORDING_PAUSED,
    obs_frontend_event_OBS_FRONTEND_EVENT_RECORDING_STARTED,
    obs_frontend_event_OBS_FRONTEND_EVENT_RECORDING_STARTING,
    obs_frontend_event_OBS_FRONTEND_EVENT_RECORDING_STOPPED,
    obs_frontend_event_OBS_FRONTEND_EVENT_RECORDING_STOPPING,
    obs_frontend_event_OBS_FRONTEND_EVENT_RECORDING_UNPAUSED,
    obs_frontend_event_OBS_FRONTEND_EVENT_REPLAY_BUFFER_SAVED,
    obs_frontend_event_OBS_FRONTEND_EVENT_REPLAY_BUFFER_STARTED,
    obs_frontend_event_OBS_FRONTEND_EVENT_REPLAY_BUFFER_STARTING,
    obs_frontend_event_OBS_FRONTEND_EVENT_REPLAY_BUFFER_STOPPED,
    obs_frontend_event_OBS_FRONTEND_EVENT_REPLAY_BUFFER_STOPPING,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_COLLECTION_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_COLLECTION_CHANGING,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_COLLECTION_CLEANUP,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_COLLECTION_LIST_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_COLLECTION_RENAMED,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCENE_LIST_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCREENSHOT_TAKEN,
    obs_frontend_event_OBS_FRONTEND_EVENT_SCRIPTING_SHUTDOWN,
    obs_frontend_event_OBS_FRONTEND_EVENT_STREAMING_STARTED,
    obs_frontend_event_OBS_FRONTEND_EVENT_STREAMING_STARTING,
    obs_frontend_event_OBS_FRONTEND_EVENT_STREAMING_STOPPED,
    obs_frontend_event_OBS_FRONTEND_EVENT_STREAMING_STOPPING,
    obs_frontend_event_OBS_FRONTEND_EVENT_STUDIO_MODE_DISABLED,
    obs_frontend_event_OBS_FRONTEND_EVENT_STUDIO_MODE_ENABLED,
    obs_frontend_event_OBS_FRONTEND_EVENT_TBAR_VALUE_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_THEME_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_TRANSITION_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_TRANSITION_DURATION_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_TRANSITION_LIST_CHANGED,
    obs_frontend_event_OBS_FRONTEND_EVENT_TRANSITION_STOPPED,
    obs_frontend_event_OBS_FRONTEND_EVENT_VIRTUALCAM_STARTED,
    obs_frontend_event_OBS_FRONTEND_EVENT_VIRTUALCAM_STOPPED, obs_frontend_remove_event_callback,
};

#[cfg(feature = "obs-32")]
#[allow(unused_imports)]
use obs_rs_sys::{
    obs_frontend_event_OBS_FRONTEND_EVENT_CANVAS_ADDED,
    obs_frontend_event_OBS_FRONTEND_EVENT_CANVAS_REMOVED,
};

use crate::native_enum;

native_enum!(
    /// High-level UI / engine events emitted by OBS Studio.
    ///
    /// New variants are added across OBS major versions; this enum
    /// reflects the v32 set. When built against an older OBS the unknown
    /// values are surfaced via [`from_raw`'s error path][Self::from_raw].
    Event, obs_frontend_event {
        StreamingStarting => OBS_FRONTEND_EVENT_STREAMING_STARTING,
        StreamingStarted => OBS_FRONTEND_EVENT_STREAMING_STARTED,
        StreamingStopping => OBS_FRONTEND_EVENT_STREAMING_STOPPING,
        StreamingStopped => OBS_FRONTEND_EVENT_STREAMING_STOPPED,
        RecordingStarting => OBS_FRONTEND_EVENT_RECORDING_STARTING,
        RecordingStarted => OBS_FRONTEND_EVENT_RECORDING_STARTED,
        RecordingStopping => OBS_FRONTEND_EVENT_RECORDING_STOPPING,
        RecordingStopped => OBS_FRONTEND_EVENT_RECORDING_STOPPED,
        SceneChanged => OBS_FRONTEND_EVENT_SCENE_CHANGED,
        SceneListChanged => OBS_FRONTEND_EVENT_SCENE_LIST_CHANGED,
        TransitionChanged => OBS_FRONTEND_EVENT_TRANSITION_CHANGED,
        TransitionStopped => OBS_FRONTEND_EVENT_TRANSITION_STOPPED,
        TransitionListChanged => OBS_FRONTEND_EVENT_TRANSITION_LIST_CHANGED,
        SceneCollectionChanged => OBS_FRONTEND_EVENT_SCENE_COLLECTION_CHANGED,
        SceneCollectionListChanged => OBS_FRONTEND_EVENT_SCENE_COLLECTION_LIST_CHANGED,
        ProfileChanged => OBS_FRONTEND_EVENT_PROFILE_CHANGED,
        ProfileListChanged => OBS_FRONTEND_EVENT_PROFILE_LIST_CHANGED,
        Exit => OBS_FRONTEND_EVENT_EXIT,
        ReplayBufferStarting => OBS_FRONTEND_EVENT_REPLAY_BUFFER_STARTING,
        ReplayBufferStarted => OBS_FRONTEND_EVENT_REPLAY_BUFFER_STARTED,
        ReplayBufferStopping => OBS_FRONTEND_EVENT_REPLAY_BUFFER_STOPPING,
        ReplayBufferStopped => OBS_FRONTEND_EVENT_REPLAY_BUFFER_STOPPED,
        StudioModeEnabled => OBS_FRONTEND_EVENT_STUDIO_MODE_ENABLED,
        StudioModeDisabled => OBS_FRONTEND_EVENT_STUDIO_MODE_DISABLED,
        PreviewSceneChanged => OBS_FRONTEND_EVENT_PREVIEW_SCENE_CHANGED,
        SceneCollectionCleanup => OBS_FRONTEND_EVENT_SCENE_COLLECTION_CLEANUP,
        FinishedLoading => OBS_FRONTEND_EVENT_FINISHED_LOADING,
        RecordingPaused => OBS_FRONTEND_EVENT_RECORDING_PAUSED,
        RecordingUnpaused => OBS_FRONTEND_EVENT_RECORDING_UNPAUSED,
        TransitionDurationChanged => OBS_FRONTEND_EVENT_TRANSITION_DURATION_CHANGED,
        ReplayBufferSaved => OBS_FRONTEND_EVENT_REPLAY_BUFFER_SAVED,
        VirtualcamStarted => OBS_FRONTEND_EVENT_VIRTUALCAM_STARTED,
        VirtualcamStopped => OBS_FRONTEND_EVENT_VIRTUALCAM_STOPPED,
        TbarValueChanged => OBS_FRONTEND_EVENT_TBAR_VALUE_CHANGED,
        SceneCollectionChanging => OBS_FRONTEND_EVENT_SCENE_COLLECTION_CHANGING,
        ProfileChanging => OBS_FRONTEND_EVENT_PROFILE_CHANGING,
        ScriptingShutdown => OBS_FRONTEND_EVENT_SCRIPTING_SHUTDOWN,
        ProfileRenamed => OBS_FRONTEND_EVENT_PROFILE_RENAMED,
        SceneCollectionRenamed => OBS_FRONTEND_EVENT_SCENE_COLLECTION_RENAMED,
        ThemeChanged => OBS_FRONTEND_EVENT_THEME_CHANGED,
        ScreenshotTaken => OBS_FRONTEND_EVENT_SCREENSHOT_TAKEN,
    }
);

#[cfg(feature = "obs-32")]
crate::native_enum!(
    /// Canvas-lifecycle events. Only emitted by OBS 32+.
    ///
    /// Folded into the main [`Event`] enum at runtime — these constants
    /// just exist so the underlying `obs_frontend_event` numbering stays
    /// in sync. Most callers should match on [`Event`].
    CanvasEvent, obs_frontend_event {
        CanvasAdded => OBS_FRONTEND_EVENT_CANVAS_ADDED,
        CanvasRemoved => OBS_FRONTEND_EVENT_CANVAS_REMOVED,
    }
);

/// Registers `callback` to receive [`Event`] notifications and returns a
/// guard that detaches the callback when dropped.
///
/// Unknown event values (e.g. when running against a newer OBS than this
/// crate was built for) are silently ignored.
pub fn add_event_callback<F: FnMut(Event) + 'static>(callback: F) -> EventCallbackHandle {
    let boxed: Box<Box<dyn FnMut(Event)>> = Box::new(Box::new(callback));
    let raw = Box::into_raw(boxed);
    unsafe {
        obs_frontend_add_event_callback(Some(thunk), raw as *mut c_void);
    }
    EventCallbackHandle { boxed: raw }
}

unsafe extern "C" fn thunk(event: obs_frontend_event, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let cb = unsafe { &mut *(data as *mut Box<dyn FnMut(Event)>) };
    if let Ok(ev) = Event::from_raw(event) {
        cb(ev);
    }
}

/// RAII handle for an event callback registration.
///
/// Dropping the handle removes the callback from OBS and frees the
/// closure storage. To keep the callback alive for the rest of the
/// process, call [`Self::leak`].
pub struct EventCallbackHandle {
    boxed: *mut Box<dyn FnMut(Event)>,
}

impl EventCallbackHandle {
    /// Detaches the callback without dropping the closure. The closure
    /// will remain registered for the lifetime of the OBS process.
    pub fn leak(self) {
        std::mem::forget(self);
    }
}

impl Drop for EventCallbackHandle {
    fn drop(&mut self) {
        unsafe {
            obs_frontend_remove_event_callback(Some(thunk), self.boxed as *mut c_void);
            drop(Box::from_raw(self.boxed));
        }
    }
}
