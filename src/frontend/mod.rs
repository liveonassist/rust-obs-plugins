//! Bindings to OBS Studio's [Frontend API].
//!
//! The frontend API is the surface that user-facing OBS Studio exposes to
//! plugins for integration with the application UI: querying or driving
//! recording / streaming / virtual-camera state, enumerating scenes and scene
//! collections, observing high-level events such as scene changes, hooking
//! into save/load, and so on. It is *not* part of `libobs` proper — it lives
//! in a separate `obs-frontend-api` shared library and is only present when a
//! plugin is loaded inside OBS Studio itself. Calling these functions from a
//! standalone process or from CLI consumers of `libobs` will fail or return
//! null.
//!
//! [Frontend API]: https://github.com/obsproject/obs-studio/blob/master/UI/obs-frontend-api/obs-frontend-api.h
//!
//! # Threading
//!
//! Most frontend functions must be called from OBS's UI thread. Treat the
//! types in this module as `!Send` / `!Sync` even though Rust does not
//! enforce that here — calling them off-thread is undefined behaviour at the
//! C level.
//!
//! # Lifetime of returned pointers
//!
//! Functions that return `String` (e.g. `current_profile`) and the various
//! list helpers internally call `bfree` on OBS-owned heap allocations, so
//! the values they return are independent owned Rust objects. Functions
//! that return source / output / scene / canvas references hand back
//! *new references* via the existing wrapper types (`SourceRef`,
//! `OutputRef`, etc.); their `Drop` impls release for you.
//!
//! # Qt
//!
//! Functions that exchange Qt-object pointers (main window, tools-menu
//! `QAction`, dock widgets, system tray) trade in `OwnedQtPtr` /
//! `BorrowedQtPtr` instead of bare `*mut c_void`. Constructing an
//! `OwnedQtPtr` is `unsafe` (you assert the pointer's validity and
//! ownership transfer), but the OBS-facing functions themselves are
//! safe — there's no way to call them with a `cxx::UniquePtr` whose
//! Drop will free the widget out from under OBS.
//!
//! `obs-rs` does not bundle a Qt binding. Plugin authors using `cxx-qt`
//! (or any other Qt-Rust binding) construct their `QWidget` subclass
//! there, then hand its raw pointer to us via `OwnedQtPtr`. See
//! `OwnedQtPtr`'s docs for a worked example.
//!
//! # Examples
//!
//! Toggling streaming based on the current scene:
//!
//! ```no_run
//! use obs_rs::frontend::{self, Event, EventCallbackHandle};
//!
//! // Returned guard removes the callback when dropped.
//! let _guard: EventCallbackHandle = frontend::add_event_callback(|event| {
//!     if event == Event::SceneChanged && !frontend::streaming_active() {
//!         frontend::streaming_start();
//!     }
//! });
//! ```

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::PathBuf;

use obs_sys_rs::{
    bfree, config_t, obs_frontend_defer_save_begin, obs_frontend_defer_save_end,
    obs_frontend_get_current_preview_scene, obs_frontend_get_current_profile,
    obs_frontend_get_current_profile_path, obs_frontend_get_current_record_output_path,
    obs_frontend_get_current_scene, obs_frontend_get_current_scene_collection,
    obs_frontend_get_current_transition, obs_frontend_get_global_config,
    obs_frontend_get_last_recording, obs_frontend_get_last_replay,
    obs_frontend_get_last_screenshot, obs_frontend_get_locale_string, obs_frontend_get_main_window,
    obs_frontend_get_main_window_handle, obs_frontend_get_profile_config,
    obs_frontend_get_recording_output, obs_frontend_get_replay_buffer_output,
    obs_frontend_get_streaming_output, obs_frontend_get_streaming_service,
    obs_frontend_get_system_tray, obs_frontend_get_tbar_position,
    obs_frontend_get_transition_duration, obs_frontend_get_virtualcam_output,
    obs_frontend_is_theme_dark, obs_frontend_open_projector,
    obs_frontend_open_sceneitem_edit_transform, obs_frontend_open_source_filters,
    obs_frontend_open_source_interaction, obs_frontend_open_source_properties,
    obs_frontend_pop_ui_translation, obs_frontend_preview_enabled,
    obs_frontend_preview_program_mode_active, obs_frontend_preview_program_trigger_transition,
    obs_frontend_recording_active, obs_frontend_recording_add_chapter,
    obs_frontend_recording_pause, obs_frontend_recording_paused, obs_frontend_recording_split_file,
    obs_frontend_recording_start, obs_frontend_recording_stop, obs_frontend_release_tbar,
    obs_frontend_replay_buffer_active, obs_frontend_replay_buffer_save,
    obs_frontend_replay_buffer_start, obs_frontend_replay_buffer_stop, obs_frontend_reset_video,
    obs_frontend_save, obs_frontend_save_streaming_service, obs_frontend_set_current_preview_scene,
    obs_frontend_set_current_profile, obs_frontend_set_current_scene,
    obs_frontend_set_current_scene_collection, obs_frontend_set_current_transition,
    obs_frontend_set_preview_enabled, obs_frontend_set_preview_program_mode,
    obs_frontend_set_streaming_service, obs_frontend_set_tbar_position,
    obs_frontend_set_transition_duration, obs_frontend_start_virtualcam,
    obs_frontend_stop_virtualcam, obs_frontend_streaming_active, obs_frontend_streaming_start,
    obs_frontend_streaming_stop, obs_frontend_take_screenshot, obs_frontend_take_source_screenshot,
    obs_frontend_virtualcam_active, obs_service_t,
};
#[cfg(any(feature = "obs-31", feature = "obs-32"))]
use obs_sys_rs::{obs_frontend_get_app_config, obs_frontend_get_user_config};

use crate::source::SourceRef;
use crate::wrapper::PtrWrapper;

mod callbacks;
mod event;
mod lists;
mod qt;

#[cfg(any(feature = "obs-31", feature = "obs-32"))]
mod canvas;

pub use callbacks::*;
#[cfg(any(feature = "obs-31", feature = "obs-32"))]
pub use canvas::*;
pub use event::*;
pub use lists::*;
pub use qt::*;

/// Opaque pointer to an `obs_service_t`.
///
/// There is no safe `ServiceRef` wrapper yet; if you need to call into
/// `obs_service_*` functions, do so manually via [`obs_sys_rs`].
pub type ServiceHandle = *mut obs_service_t;

/// Opaque pointer to an OBS `config_t`.
///
/// `config_t` lifetimes are managed entirely by OBS — these pointers are
/// borrowed and must not be released by callers.
pub type ConfigHandle = *mut config_t;

// ---------------------------------------------------------------------------
// Streaming
// ---------------------------------------------------------------------------

/// Asks OBS Studio to start the configured streaming output.
///
/// Equivalent to clicking "Start Streaming" in the UI. Returns immediately;
/// listen for [`Event::StreamingStarting`] / [`Event::StreamingStarted`] /
/// [`Event::StreamingStopped`] to track progress.
pub fn streaming_start() {
    unsafe { obs_frontend_streaming_start() }
}

/// Asks OBS Studio to stop the active streaming output.
///
/// No-op if streaming is already stopped.
pub fn streaming_stop() {
    unsafe { obs_frontend_streaming_stop() }
}

/// Whether OBS is currently streaming.
pub fn streaming_active() -> bool {
    unsafe { obs_frontend_streaming_active() }
}

/// Returns the active streaming output, if any.
///
/// Equivalent to [`std::env::var`] — `None` if streaming is not configured.
pub fn streaming_output() -> Option<crate::output::OutputRef> {
    let ptr = unsafe { obs_frontend_get_streaming_output() };
    unsafe { crate::output::OutputRef::from_raw_unchecked(ptr) }
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// Starts the configured recording output.
pub fn recording_start() {
    unsafe { obs_frontend_recording_start() }
}

/// Stops the active recording output.
pub fn recording_stop() {
    unsafe { obs_frontend_recording_stop() }
}

/// Whether OBS is currently recording.
pub fn recording_active() -> bool {
    unsafe { obs_frontend_recording_active() }
}

/// Pauses or resumes the active recording.
///
/// Has no effect if the active output does not support pausing.
pub fn recording_pause(pause: bool) {
    unsafe { obs_frontend_recording_pause(pause) }
}

/// Whether the active recording is currently paused.
pub fn recording_paused() -> bool {
    unsafe { obs_frontend_recording_paused() }
}

/// Splits the current recording file at this point.
///
/// Returns `false` if splitting is not supported by the current output or if
/// no recording is active.
pub fn recording_split_file() -> bool {
    unsafe { obs_frontend_recording_split_file() }
}

/// Adds a chapter marker to the current recording.
///
/// `name` may be `None` to use the default chapter name (an incrementing
/// number). Returns `false` if no recording is active or the output does
/// not support chapter markers.
pub fn recording_add_chapter(name: Option<&str>) -> Result<bool, std::ffi::NulError> {
    let cstr = name.map(CString::new).transpose()?;
    let ptr = cstr.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
    Ok(unsafe { obs_frontend_recording_add_chapter(ptr) })
}

/// Returns the currently-recording output, if any.
pub fn recording_output() -> Option<crate::output::OutputRef> {
    let ptr = unsafe { obs_frontend_get_recording_output() };
    unsafe { crate::output::OutputRef::from_raw_unchecked(ptr) }
}

/// Path to the most recently completed recording, or `None` if no recording
/// has been completed in this session.
pub fn last_recording() -> Option<PathBuf> {
    unsafe { take_bfree_string(obs_frontend_get_last_recording()) }.map(PathBuf::from)
}

/// Output directory currently configured for recordings, or `None` if not
/// applicable (e.g. when configured to record to a custom path).
pub fn current_record_output_path() -> Option<PathBuf> {
    unsafe { take_bfree_string(obs_frontend_get_current_record_output_path()) }.map(PathBuf::from)
}

// ---------------------------------------------------------------------------
// Replay buffer
// ---------------------------------------------------------------------------

/// Starts the replay buffer.
pub fn replay_buffer_start() {
    unsafe { obs_frontend_replay_buffer_start() }
}

/// Saves the current replay buffer contents to disk.
pub fn replay_buffer_save() {
    unsafe { obs_frontend_replay_buffer_save() }
}

/// Stops the replay buffer.
pub fn replay_buffer_stop() {
    unsafe { obs_frontend_replay_buffer_stop() }
}

/// Whether the replay buffer is currently running.
pub fn replay_buffer_active() -> bool {
    unsafe { obs_frontend_replay_buffer_active() }
}

/// Replay buffer output, if active.
pub fn replay_buffer_output() -> Option<crate::output::OutputRef> {
    let ptr = unsafe { obs_frontend_get_replay_buffer_output() };
    unsafe { crate::output::OutputRef::from_raw_unchecked(ptr) }
}

/// Path to the most recently saved replay, or `None` if no replay has been
/// saved this session.
pub fn last_replay() -> Option<PathBuf> {
    unsafe { take_bfree_string(obs_frontend_get_last_replay()) }.map(PathBuf::from)
}

// ---------------------------------------------------------------------------
// Virtual camera
// ---------------------------------------------------------------------------

/// Starts the virtual camera output.
pub fn virtualcam_start() {
    unsafe { obs_frontend_start_virtualcam() }
}

/// Stops the virtual camera output.
pub fn virtualcam_stop() {
    unsafe { obs_frontend_stop_virtualcam() }
}

/// Whether the virtual camera is currently active.
pub fn virtualcam_active() -> bool {
    unsafe { obs_frontend_virtualcam_active() }
}

/// Virtual-camera output, if active.
pub fn virtualcam_output() -> Option<crate::output::OutputRef> {
    let ptr = unsafe { obs_frontend_get_virtualcam_output() };
    unsafe { crate::output::OutputRef::from_raw_unchecked(ptr) }
}

// ---------------------------------------------------------------------------
// Scenes
// ---------------------------------------------------------------------------

/// Currently active program scene, or `None` if no scene is loaded.
///
/// In studio mode this returns the scene being broadcast — see
/// [`current_preview_scene`] for the preview scene.
pub fn current_scene() -> Option<SourceRef> {
    let ptr = unsafe { obs_frontend_get_current_scene() };
    unsafe { SourceRef::from_raw_unchecked(ptr) }
}

/// Switches the program scene.
pub fn set_current_scene(scene: &SourceRef) {
    unsafe { obs_frontend_set_current_scene(scene.as_ptr() as *mut _) }
}

/// Currently active preview scene, when in studio mode. Always `None`
/// outside studio mode.
pub fn current_preview_scene() -> Option<SourceRef> {
    let ptr = unsafe { obs_frontend_get_current_preview_scene() };
    unsafe { SourceRef::from_raw_unchecked(ptr) }
}

/// Switches the preview scene (studio mode only).
pub fn set_current_preview_scene(scene: &SourceRef) {
    unsafe { obs_frontend_set_current_preview_scene(scene.as_ptr() as *mut _) }
}

// ---------------------------------------------------------------------------
// Transitions / T-bar
// ---------------------------------------------------------------------------

/// Currently selected transition source, or `None` if none is configured.
pub fn current_transition() -> Option<SourceRef> {
    let ptr = unsafe { obs_frontend_get_current_transition() };
    unsafe { SourceRef::from_raw_unchecked(ptr) }
}

/// Sets the current transition source.
pub fn set_current_transition(transition: &SourceRef) {
    unsafe { obs_frontend_set_current_transition(transition.as_ptr() as *mut _) }
}

/// Default transition duration, in milliseconds.
pub fn transition_duration_ms() -> i32 {
    unsafe { obs_frontend_get_transition_duration() as i32 }
}

/// Sets the default transition duration, in milliseconds.
pub fn set_transition_duration_ms(duration_ms: i32) {
    unsafe { obs_frontend_set_transition_duration(duration_ms as c_int) }
}

/// Returns the studio-mode T-bar position in the range `0..=100`.
pub fn tbar_position() -> i32 {
    unsafe { obs_frontend_get_tbar_position() as i32 }
}

/// Sets the studio-mode T-bar position. Valid range is `0..=100`.
pub fn set_tbar_position(position: i32) {
    unsafe { obs_frontend_set_tbar_position(position as c_int) }
}

/// Releases the T-bar after a manual move, allowing OBS to finish the
/// transition.
pub fn release_tbar() {
    unsafe { obs_frontend_release_tbar() }
}

/// Whether studio (preview/program) mode is enabled.
pub fn preview_program_mode_active() -> bool {
    unsafe { obs_frontend_preview_program_mode_active() }
}

/// Toggles studio (preview/program) mode.
pub fn set_preview_program_mode(enable: bool) {
    unsafe { obs_frontend_set_preview_program_mode(enable) }
}

/// Triggers a transition from preview to program (studio mode only).
pub fn preview_program_trigger_transition() {
    unsafe { obs_frontend_preview_program_trigger_transition() }
}

/// Whether the preview pane is enabled.
pub fn preview_enabled() -> bool {
    unsafe { obs_frontend_preview_enabled() }
}

/// Enables or disables the preview pane.
pub fn set_preview_enabled(enable: bool) {
    unsafe { obs_frontend_set_preview_enabled(enable) }
}

// ---------------------------------------------------------------------------
// Scene collections
// ---------------------------------------------------------------------------

/// Name of the active scene collection, or `None` if none.
pub fn current_scene_collection() -> Option<String> {
    unsafe { take_bfree_string(obs_frontend_get_current_scene_collection()) }
}

/// Switches to the named scene collection.
pub fn set_current_scene_collection(name: &str) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(name)?;
    unsafe { obs_frontend_set_current_scene_collection(cstr.as_ptr()) };
    Ok(())
}

// ---------------------------------------------------------------------------
// Profiles
// ---------------------------------------------------------------------------

/// Name of the active profile, or `None` if no profile is loaded.
pub fn current_profile() -> Option<String> {
    unsafe { take_bfree_string(obs_frontend_get_current_profile()) }
}

/// Filesystem path of the active profile.
pub fn current_profile_path() -> Option<PathBuf> {
    unsafe { take_bfree_string(obs_frontend_get_current_profile_path()) }.map(PathBuf::from)
}

/// Switches to the named profile.
pub fn set_current_profile(name: &str) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(name)?;
    unsafe { obs_frontend_set_current_profile(cstr.as_ptr()) };
    Ok(())
}

/// Creates a new empty profile with the given name.
pub fn create_profile(name: &str) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(name)?;
    unsafe { obs_sys_rs::obs_frontend_create_profile(cstr.as_ptr()) };
    Ok(())
}

/// Duplicates the current profile under a new name.
pub fn duplicate_profile(name: &str) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(name)?;
    unsafe { obs_sys_rs::obs_frontend_duplicate_profile(cstr.as_ptr()) };
    Ok(())
}

/// Deletes the named profile. Has no effect if the profile is currently
/// active.
pub fn delete_profile(name: &str) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(name)?;
    unsafe { obs_sys_rs::obs_frontend_delete_profile(cstr.as_ptr()) };
    Ok(())
}

// ---------------------------------------------------------------------------
// Save / load
// ---------------------------------------------------------------------------

/// Asks OBS to write the current scene collection to disk.
pub fn save() {
    unsafe { obs_frontend_save() }
}

/// Begins a deferred-save scope.
///
/// Save callbacks invoked between [`defer_save_begin`] and [`defer_save_end`]
/// are coalesced into a single save. Always pair with [`defer_save_end`];
/// consider [`DeferSaveGuard`] for an RAII version.
pub fn defer_save_begin() {
    unsafe { obs_frontend_defer_save_begin() }
}

/// Closes a deferred-save scope started with [`defer_save_begin`].
pub fn defer_save_end() {
    unsafe { obs_frontend_defer_save_end() }
}

/// RAII guard for [`defer_save_begin`] / [`defer_save_end`].
///
/// ```no_run
/// use obs_rs::frontend::DeferSaveGuard;
/// {
///     let _guard = DeferSaveGuard::new();
///     // multiple settings changes that would each trigger a save…
/// } // single save flushed here
/// ```
pub struct DeferSaveGuard(());

impl DeferSaveGuard {
    /// Calls [`defer_save_begin`].
    pub fn new() -> Self {
        defer_save_begin();
        Self(())
    }
}

impl Default for DeferSaveGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DeferSaveGuard {
    fn drop(&mut self) {
        defer_save_end();
    }
}

// ---------------------------------------------------------------------------
// Streaming service
// ---------------------------------------------------------------------------

/// Returns the currently-configured streaming service handle.
///
/// This is an unsafe handle because there is no `ServiceRef` wrapper yet.
/// The pointer is owned by OBS and must not be released.
///
/// # Safety
///
/// Caller must not retain the pointer past the active session and must not
/// call `obs_service_release` on it.
pub unsafe fn streaming_service() -> ServiceHandle {
    unsafe { obs_frontend_get_streaming_service() }
}

/// Replaces the configured streaming service.
///
/// # Safety
///
/// `service` must be a valid `obs_service_t*` created by `obs_service_create`
/// (or an existing handle obtained from OBS). Ownership semantics follow the
/// underlying C API.
pub unsafe fn set_streaming_service(service: ServiceHandle) {
    unsafe { obs_frontend_set_streaming_service(service) }
}

/// Persists changes to the streaming service.
pub fn save_streaming_service() {
    unsafe { obs_frontend_save_streaming_service() }
}

// ---------------------------------------------------------------------------
// Configs
// ---------------------------------------------------------------------------

/// Returns the active profile's `config_t`.
///
/// # Safety
///
/// The returned pointer is owned by OBS and only valid as long as the
/// profile remains active. Do not free it.
pub unsafe fn profile_config() -> ConfigHandle {
    unsafe { obs_frontend_get_profile_config() }
}

/// **Deprecated** in OBS 31+. Use [`app_config`] / [`user_config`] instead.
///
/// # Safety
///
/// Same as [`profile_config`].
#[cfg_attr(
    any(feature = "obs-31", feature = "obs-32"),
    deprecated(note = "Use app_config or user_config (OBS 31+)")
)]
pub unsafe fn global_config() -> ConfigHandle {
    unsafe { obs_frontend_get_global_config() }
}

/// Application-level configuration shared across users. **OBS 31+ only.**
///
/// # Safety
///
/// Same as [`profile_config`].
#[cfg(any(feature = "obs-31", feature = "obs-32"))]
pub unsafe fn app_config() -> ConfigHandle {
    unsafe { obs_frontend_get_app_config() }
}

/// User-level configuration. **OBS 31+ only.**
///
/// # Safety
///
/// Same as [`profile_config`].
#[cfg(any(feature = "obs-31", feature = "obs-32"))]
pub unsafe fn user_config() -> ConfigHandle {
    unsafe { obs_frontend_get_user_config() }
}

// ---------------------------------------------------------------------------
// Misc UI
// ---------------------------------------------------------------------------

/// Whether the current OBS theme is dark-mode.
pub fn is_theme_dark() -> bool {
    unsafe { obs_frontend_is_theme_dark() }
}

/// Captures a screenshot of the current program output and saves it to the
/// configured screenshot directory. Listen for [`Event::ScreenshotTaken`]
/// to know when it's complete.
pub fn take_screenshot() {
    unsafe { obs_frontend_take_screenshot() }
}

/// Captures a screenshot of just the given source.
pub fn take_source_screenshot(source: &SourceRef) {
    unsafe { obs_frontend_take_source_screenshot(source.as_ptr() as *mut _) }
}

/// Path to the most recently captured screenshot in this session.
pub fn last_screenshot() -> Option<PathBuf> {
    unsafe { take_bfree_string(obs_frontend_get_last_screenshot()) }.map(PathBuf::from)
}

/// Opens an OBS projector window.
///
/// `kind` is one of `"Preview"`, `"Source"`, `"Scene"`, `"StudioProgram"`,
/// `"Multiview"` (case-sensitive). `monitor` selects a monitor by index, or
/// `-1` to use a windowed projector. `geometry` is a Qt
/// `QByteArray::toBase64()` window-geometry string, or empty for default.
pub fn open_projector(
    kind: &str,
    monitor: i32,
    geometry: &str,
    name: &str,
) -> Result<(), std::ffi::NulError> {
    let kind = CString::new(kind)?;
    let geom = CString::new(geometry)?;
    let name = CString::new(name)?;
    unsafe {
        obs_frontend_open_projector(
            kind.as_ptr(),
            monitor as c_int,
            geom.as_ptr(),
            name.as_ptr(),
        )
    };
    Ok(())
}

/// Asks OBS to re-initialize the video pipeline. Equivalent to a
/// settings/apply round-trip.
pub fn reset_video() {
    unsafe { obs_frontend_reset_video() }
}

/// Opens the source-properties dialog for the given source.
pub fn open_source_properties(source: &SourceRef) {
    unsafe { obs_frontend_open_source_properties(source.as_ptr() as *mut _) }
}

/// Opens the source-filters dialog for the given source.
pub fn open_source_filters(source: &SourceRef) {
    unsafe { obs_frontend_open_source_filters(source.as_ptr() as *mut _) }
}

/// Opens the source-interaction window for the given source (only valid
/// for sources flagged `OBS_SOURCE_INTERACTION`).
pub fn open_source_interaction(source: &SourceRef) {
    unsafe { obs_frontend_open_source_interaction(source.as_ptr() as *mut _) }
}

/// Opens the transform-edit dialog for the given scene item.
pub fn open_sceneitem_edit_transform(item: &crate::source::scene::SceneItemRef) {
    unsafe { obs_frontend_open_sceneitem_edit_transform(item.as_ptr() as *mut _) }
}

/// Looks up a string in OBS's UI locale.
///
/// Returns `None` if the string is not found. The lookup is case-sensitive
/// and uses OBS's bundled translations rather than Qt's, so this is
/// distinct from `QObject::tr`.
pub fn locale_string(key: &str) -> Option<String> {
    let cstr = CString::new(key).ok()?;
    let ptr = unsafe { obs_frontend_get_locale_string(cstr.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(
            unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned(),
        )
    }
}

// ---------------------------------------------------------------------------
// UI translation push/pop
// ---------------------------------------------------------------------------

/// Pops a UI-translation scope previously pushed by
/// `obs_frontend_push_ui_translation`.
///
/// The push side of this API takes a callback function; we don't currently
/// expose a Rust wrapper for it because it sees only `const char *` input
/// strings. If you need it, call it via [`obs_sys_rs`] directly.
pub fn pop_ui_translation() {
    unsafe { obs_frontend_pop_ui_translation() }
}

// ---------------------------------------------------------------------------
// Qt — typed handles via OwnedQtPtr / BorrowedQtPtr
// ---------------------------------------------------------------------------
//
// The OBS frontend trades in raw `QWidget*` / `QAction*` / `QMainWindow*`
// pointers. We expose them through [`OwnedQtPtr`] (ownership transfer to
// OBS) and [`BorrowedQtPtr`] (borrow from OBS) instead of bare `*mut
// c_void`, so a stray Drop on a `cxx::UniquePtr` can't free a widget OBS
// still owns. See [`qt`] module docs for the integration pattern.

/// Pointer to the OBS Studio main window (`QMainWindow*`), or `None` if
/// the OBS frontend is not initialized.
///
/// The returned [`BorrowedQtPtr`] is valid for the lifetime of the OBS
/// process. Use [`BorrowedQtPtr::as_ptr`] to obtain a `*mut c_void` for
/// your `cxx-qt` (or other) Qt binding.
pub fn main_window() -> Option<BorrowedQtPtr<'static>> {
    unsafe { BorrowedQtPtr::from_raw(obs_frontend_get_main_window()) }
}

/// Native (platform) handle of the main window: `HWND` on Windows,
/// `NSWindow*` on macOS, `xcb_window_t` / `wl_surface*` on Linux.
///
/// Returns `None` if the frontend is not initialized.
pub fn main_window_handle() -> Option<BorrowedQtPtr<'static>> {
    unsafe { BorrowedQtPtr::from_raw(obs_frontend_get_main_window_handle()) }
}

/// Pointer to the OBS system tray icon (`QSystemTrayIcon*`), or `None`
/// if OBS is configured without a tray.
pub fn system_tray() -> Option<BorrowedQtPtr<'static>> {
    unsafe { BorrowedQtPtr::from_raw(obs_frontend_get_system_tray()) }
}

/// Adds a new entry under OBS's "Tools" menu, returning the
/// `QAction*` that OBS retains ownership of.
///
/// Use the returned [`BorrowedQtPtr`] with your Qt binding to set
/// properties (`setText`, `setEnabled`) or connect to its `triggered`
/// signal. Do not call `delete` on it.
///
/// Returns `Ok(None)` if OBS is not yet initialized (the frontend
/// returned a null pointer); callers should typically defer registration
/// until [`Event::FinishedLoading`].
pub fn add_tools_menu_qaction(
    name: &str,
) -> Result<Option<BorrowedQtPtr<'static>>, std::ffi::NulError> {
    let cstr = CString::new(name)?;
    let raw = unsafe { obs_sys_rs::obs_frontend_add_tools_menu_qaction(cstr.as_ptr()) };
    Ok(unsafe { BorrowedQtPtr::from_raw(raw) })
}

/// Adds a Tools-menu entry that invokes `callback` when triggered.
///
/// The callback runs on the Qt UI thread. The closure is **leaked
/// intentionally** — OBS retains a function pointer to it for the
/// lifetime of the process and provides no removal API. Returning a
/// handle that could free the closure on drop would create a
/// use-after-free hazard, so this function returns `()`.
pub fn add_tools_menu_item<F: FnMut() + 'static>(
    name: &str,
    callback: F,
) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(name)?;
    let boxed: Box<Box<dyn FnMut()>> = Box::new(Box::new(callback));
    // Intentional leak: OBS holds onto this Box pointer for the rest of
    // the process. Freeing it would cause a UAF on the next click.
    let raw = Box::into_raw(boxed);
    unsafe {
        obs_sys_rs::obs_frontend_add_tools_menu_item(
            cstr.as_ptr(),
            Some(tools_menu_thunk),
            raw as *mut c_void,
        )
    };
    Ok(())
}

unsafe extern "C" fn tools_menu_thunk(data: *mut c_void) {
    let cb = unsafe { &mut *(data as *mut Box<dyn FnMut()>) };
    cb();
}

/// Adds a `QWidget*` as a docked panel in OBS's main window.
///
/// `widget` is an [`OwnedQtPtr`] wrapping a `QWidget*`. OBS re-parents
/// the widget into a freshly constructed `QDockWidget` and owns it
/// thereafter. Returns `Ok(true)` on success, `Ok(false)` if a dock
/// with the same `id` already exists.
///
/// # Example (with `cxx-qt` feature)
///
/// ```ignore
/// let dock: cxx::UniquePtr<MyDock> = my_dock::create();
/// let owned = unsafe { OwnedQtPtr::from_cxx_unique_ptr(dock) }
///     .expect("dock was null");
/// add_dock_by_id("plugin-dock", "Plugin Dock", owned)?;
/// ```
///
/// # Example (without the feature)
///
/// ```ignore
/// let dock: cxx::UniquePtr<MyDock> = my_dock::create();
/// let raw = dock.as_ref().unwrap() as *const _ as *mut c_void;
/// let owned = unsafe { OwnedQtPtr::from_smart_ptr(raw, dock) };
/// add_dock_by_id("plugin-dock", "Plugin Dock", owned)?;
/// ```
pub fn add_dock_by_id(
    id: &str,
    title: &str,
    widget: OwnedQtPtr,
) -> Result<bool, std::ffi::NulError> {
    let id = CString::new(id)?;
    let title = CString::new(title)?;
    Ok(unsafe {
        obs_sys_rs::obs_frontend_add_dock_by_id(id.as_ptr(), title.as_ptr(), widget.as_ptr())
    })
}

/// Removes a dock previously added with [`add_dock_by_id`] /
/// [`add_custom_qdock`].
pub fn remove_dock(id: &str) -> Result<(), std::ffi::NulError> {
    let cstr = CString::new(id)?;
    unsafe { obs_sys_rs::obs_frontend_remove_dock(cstr.as_ptr()) };
    Ok(())
}

/// Adds a fully-formed `QDockWidget*` to the OBS UI.
///
/// `dock` is an [`OwnedQtPtr`] wrapping a `QDockWidget*`. Use this when
/// you've subclassed `QDockWidget` (or otherwise constructed one with
/// custom title-bar / behavior) and want OBS to install it directly,
/// rather than letting OBS wrap a plain `QWidget` for you.
pub fn add_custom_qdock(id: &str, dock: OwnedQtPtr) -> Result<bool, std::ffi::NulError> {
    let id = CString::new(id)?;
    Ok(unsafe { obs_sys_rs::obs_frontend_add_custom_qdock(id.as_ptr(), dock.as_ptr()) })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Consumes a `*mut c_char` returned from a `bfree`-managed OBS API,
/// returning an owned `String`. The pointer is freed with `bfree` regardless
/// of whether the contents are valid UTF-8.
///
/// # Safety
///
/// `ptr` must be either null or a pointer obtained from an OBS API whose
/// documented contract is "free with bfree".
pub(crate) unsafe fn take_bfree_string(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { bfree(ptr as *mut c_void) };
    Some(s)
}
