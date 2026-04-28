use std::ffi::CString;

use crate::hotkey::{Hotkey, HotkeyCallbacks};
use crate::media::audio::AudioRef;
use crate::prelude::DataObj;
use obs_sys_rs::obs_get_audio;

/// Process-global context provided to source callbacks.
///
/// Acts as a capability token: holding a `GlobalContext` indicates the
/// caller is running inside an OBS callback and can safely access
/// process-wide resources such as the global audio output.
pub struct GlobalContext;

/// Per-call context for [`VideoRenderSource::video_render`].
///
/// Reserved for future use; instances carry no state today.
///
/// [`VideoRenderSource::video_render`]: super::traits::VideoRenderSource::video_render
pub struct VideoRenderContext;

impl GlobalContext {
    /// Invokes `func` with a borrowed reference to the global audio
    /// output.
    pub fn with_audio<T, F: FnOnce(&AudioRef) -> T>(&self, func: F) -> T {
        let audio = unsafe { AudioRef::from_raw(obs_get_audio()) };
        func(&audio)
    }
}

impl Default for VideoRenderContext {
    fn default() -> Self {
        Self
    }
}

impl Default for GlobalContext {
    fn default() -> Self {
        Self
    }
}

/// Construction-time context handed to [`Sourceable::create`].
///
/// Carries the initial [`DataObj`] settings supplied by OBS, a borrow of
/// the [`GlobalContext`], and lets the source register hotkeys that fire
/// callbacks with mutable access to its per-instance state.
///
/// [`Sourceable::create`]: super::traits::Sourceable::create
pub struct CreatableSourceContext<'a, D> {
    pub(crate) hotkey_callbacks: HotkeyCallbacks<D>,
    /// Initial source settings.
    pub settings: DataObj<'a>,
    /// Borrowed handle to the global OBS context.
    pub global: &'a mut GlobalContext,
}

impl<'a, D> CreatableSourceContext<'a, D> {
    pub(crate) unsafe fn from_raw(settings: DataObj<'a>, global: &'a mut GlobalContext) -> Self {
        Self {
            hotkey_callbacks: Default::default(),
            settings,
            global,
        }
    }

    /// Registers a hotkey that invokes `func` while the source instance is
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

    /// Convenience forwarder to [`GlobalContext::with_audio`].
    pub fn with_audio<T, F: FnOnce(&AudioRef) -> T>(&self, func: F) -> T {
        self.global.with_audio(func)
    }
}
