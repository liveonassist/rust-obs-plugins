//! Save, preload, and undo/redo callback registration.
//!
//! These complement the [event][super::event] callbacks and let plugins
//! plug into OBS's persistence model (the per-scene-collection
//! `obs_data_t` blob) and the global undo/redo stack.

use std::ffi::CString;
use std::os::raw::{c_char, c_void};

use obs_sys_rs::{
    obs_data_addref, obs_data_release, obs_data_t, obs_frontend_add_preload_callback,
    obs_frontend_add_save_callback, obs_frontend_add_undo_redo_action,
    obs_frontend_remove_preload_callback, obs_frontend_remove_save_callback,
};

use crate::data::DataObj;
use crate::wrapper::PtrWrapper;

// ---------------------------------------------------------------------------
// Save / preload callbacks
// ---------------------------------------------------------------------------

type BoxedSaveCb = Box<dyn FnMut(&mut DataObj<'_>, SaveDirection)>;

/// Direction of a save callback.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SaveDirection {
    /// OBS is asking the callback to read settings out of the blob.
    Loading,
    /// OBS is asking the callback to write settings into the blob.
    Saving,
}

/// Registers a callback invoked when OBS persists / restores the scene
/// collection.
///
/// The callback receives:
/// - a [`DataObj`] representing the scene-collection blob (read it on
///   [`SaveDirection::Loading`], mutate it on [`SaveDirection::Saving`])
/// - the [`SaveDirection`] selector
///
/// The returned handle removes the callback when dropped.
///
/// # Note on the `DataObj`
///
/// The [`DataObj`] passed to the closure is a *new reference* — dropping
/// it inside the closure releases your reference, not OBS's. Don't store
/// it past the callback's lifetime; the underlying blob will be reused.
pub fn add_save_callback<F: FnMut(&mut DataObj<'_>, SaveDirection) + 'static>(
    callback: F,
) -> SaveCallbackHandle {
    let boxed: Box<BoxedSaveCb> = Box::new(Box::new(callback));
    let raw = Box::into_raw(boxed);
    unsafe {
        obs_frontend_add_save_callback(Some(save_thunk), raw as *mut c_void);
    }
    SaveCallbackHandle {
        boxed: raw,
        kind: SaveKind::Save,
    }
}

/// Registers a callback invoked just before a scene collection finishes
/// loading.
///
/// Mechanics are otherwise identical to [`add_save_callback`].
pub fn add_preload_callback<F: FnMut(&mut DataObj<'_>, SaveDirection) + 'static>(
    callback: F,
) -> SaveCallbackHandle {
    let boxed: Box<BoxedSaveCb> = Box::new(Box::new(callback));
    let raw = Box::into_raw(boxed);
    unsafe {
        obs_frontend_add_preload_callback(Some(save_thunk), raw as *mut c_void);
    }
    SaveCallbackHandle {
        boxed: raw,
        kind: SaveKind::Preload,
    }
}

unsafe extern "C" fn save_thunk(data: *mut obs_data_t, saving: bool, priv_: *mut c_void) {
    if priv_.is_null() || data.is_null() {
        return;
    }
    let cb = unsafe { &mut *(priv_ as *mut BoxedSaveCb) };
    // OBS keeps its own reference for the duration of the call. Take an
    // additional ref so the wrapper's `Drop` (release) is balanced.
    unsafe { obs_data_addref(data) };
    let dir = if saving {
        SaveDirection::Saving
    } else {
        SaveDirection::Loading
    };
    if let Some(mut wrapped) = unsafe { DataObj::from_raw_unchecked(data) } {
        cb(&mut wrapped, dir);
        // wrapped is dropped here, balancing the addref above.
    } else {
        // from_raw_unchecked returned None — release manually so we don't
        // leak the addref'd ref.
        unsafe { obs_data_release(data) };
    }
}

#[derive(Copy, Clone)]
enum SaveKind {
    Save,
    Preload,
}

/// RAII handle for a [`add_save_callback`] / [`add_preload_callback`]
/// registration.
pub struct SaveCallbackHandle {
    boxed: *mut BoxedSaveCb,
    kind: SaveKind,
}

impl SaveCallbackHandle {
    /// Detaches the callback without dropping the closure (it remains
    /// registered for the lifetime of the process).
    pub fn leak(self) {
        std::mem::forget(self);
    }
}

impl Drop for SaveCallbackHandle {
    fn drop(&mut self) {
        unsafe {
            match self.kind {
                SaveKind::Save => {
                    obs_frontend_remove_save_callback(Some(save_thunk), self.boxed as *mut c_void)
                }
                SaveKind::Preload => obs_frontend_remove_preload_callback(
                    Some(save_thunk),
                    self.boxed as *mut c_void,
                ),
            }
            drop(Box::from_raw(self.boxed));
        }
    }
}

// ---------------------------------------------------------------------------
// Undo / redo
// ---------------------------------------------------------------------------

/// Function-pointer signature accepted by [`add_undo_redo_action`].
///
/// OBS's `undo_redo_cb` does not receive a `void *priv_data` slot, which
/// means capturing closures cannot be safely dispatched. Plugins that need
/// state should encode it inside `data` (which OBS round-trips
/// verbatim) — typically as JSON.
pub type UndoRedoCallback = unsafe extern "C" fn(data: *const c_char);

/// Adds a single entry to OBS's global undo/redo stack.
///
/// `name` is shown in the OBS Edit menu. `undo_data` and `redo_data` are
/// arbitrary plugin-defined byte strings that OBS passes back verbatim
/// when the user invokes Undo or Redo. `repeatable` allows the action to
/// be repeated (e.g. via `Ctrl+Y` after another action of the same kind).
///
/// The callbacks must be plain function pointers because the underlying
/// `undo_redo_cb` signature has no user-data slot. To pass state, encode
/// it into `undo_data` / `redo_data`.
///
/// # Example
///
/// ```ignore
/// use std::ffi::CStr;
/// use obs_rs::frontend::add_undo_redo_action;
///
/// unsafe extern "C" fn on_undo(data: *const std::os::raw::c_char) {
///     let s = unsafe { CStr::from_ptr(data) }.to_string_lossy();
///     // ... act on encoded payload ...
/// }
/// unsafe extern "C" fn on_redo(data: *const std::os::raw::c_char) {
///     let s = unsafe { CStr::from_ptr(data) }.to_string_lossy();
///     // ...
/// }
///
/// add_undo_redo_action("My change", on_undo, on_redo, "{\"v\":1}", "{\"v\":1}", false)
///     .expect("nul in data");
/// ```
pub fn add_undo_redo_action(
    name: &str,
    undo: UndoRedoCallback,
    redo: UndoRedoCallback,
    undo_data: &str,
    redo_data: &str,
    repeatable: bool,
) -> Result<(), std::ffi::NulError> {
    let cname = CString::new(name)?;
    let cundo = CString::new(undo_data)?;
    let credo = CString::new(redo_data)?;
    unsafe {
        obs_frontend_add_undo_redo_action(
            cname.as_ptr(),
            Some(undo),
            Some(redo),
            cundo.as_ptr(),
            credo.as_ptr(),
            repeatable,
        );
    }
    Ok(())
}
