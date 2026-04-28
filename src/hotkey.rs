use std::ffi::CString;

use obs_sys_rs::{obs_hotkey_get_id, obs_hotkey_id, obs_hotkey_t};

/// Internal collection of hotkey registrations queued during plugin
/// instance construction.
///
/// Each entry pairs a persistent identifier and localized description
/// with the closure that fires when the hotkey is triggered. The closure
/// receives a mutable reference to the live [`Hotkey`] state and to the
/// owning instance (the type parameter `T`).
pub type HotkeyCallbacks<T> = Vec<(CString, CString, Box<dyn FnMut(&mut Hotkey, &mut T)>)>;

/// A live OBS hotkey passed to a registered callback.
///
/// `Hotkey` carries the press/release state of the triggering event and
/// exposes the underlying hotkey identifier for callers that need to
/// disambiguate between multiple hotkeys sharing a single closure.
pub struct Hotkey {
    key: *mut obs_hotkey_t,
    /// `true` if the event was a key press, `false` for a release.
    pub pressed: bool,
}

impl Hotkey {
    pub(crate) unsafe fn from_raw(key: *mut obs_hotkey_t, pressed: bool) -> Self {
        Self { key, pressed }
    }

    /// Returns the hotkey's libobs identifier.
    pub fn id(&self) -> obs_hotkey_id {
        unsafe { obs_hotkey_get_id(self.key) }
    }
}
