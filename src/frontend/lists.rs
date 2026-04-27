//! Bulk enumeration of frontend resources: scenes, transitions, scene
//! collections, profiles, and (on OBS 31+) canvases.
//!
//! The C API exposes these as either:
//! - `char **` arrays where the entire allocation is a single `bfree`-able
//!   block of pointer + string memory; or
//! - `obs_frontend_source_list` / `obs_frontend_canvas_list` darray
//!   structs, where each element holds a refcount that the caller is
//!   responsible for releasing.
//!
//! All helpers here translate to plain owned [`Vec<String>`] /
//! [`Vec<SourceRef>`] / [`Vec<CanvasRef>`] values whose Drop impls release
//! the underlying OBS resources correctly.

use std::ffi::CStr;
use std::os::raw::{c_char, c_void};

use obs_rs_sys::{
    bfree, obs_frontend_get_profiles, obs_frontend_get_scene_collections,
    obs_frontend_get_scene_names, obs_frontend_get_scenes, obs_frontend_get_transitions,
    obs_frontend_source_list, obs_source_t,
};

use crate::source::SourceRef;
use crate::wrapper::PtrWrapper;

/// Names of every scene in the active scene collection.
pub fn scene_names() -> Vec<String> {
    unsafe { take_string_array(obs_frontend_get_scene_names()) }
}

/// Every scene as a [`SourceRef`].
///
/// Equivalent to `obs_frontend_get_scenes`, but each element is owned and
/// the underlying darray is freed before this returns.
pub fn scenes() -> Vec<SourceRef> {
    unsafe { drain_source_list(|list| obs_frontend_get_scenes(list)) }
}

/// Every available transition source.
pub fn transitions() -> Vec<SourceRef> {
    unsafe { drain_source_list(|list| obs_frontend_get_transitions(list)) }
}

/// All scene-collection names known to OBS.
pub fn scene_collections() -> Vec<String> {
    unsafe { take_string_array(obs_frontend_get_scene_collections()) }
}

/// All profile names known to OBS.
pub fn profiles() -> Vec<String> {
    unsafe { take_string_array(obs_frontend_get_profiles()) }
}

/// Adds a new (empty) scene collection. Returns `false` if a collection
/// with that name already exists.
pub fn add_scene_collection(name: &str) -> Result<bool, std::ffi::NulError> {
    let cstr = std::ffi::CString::new(name)?;
    Ok(unsafe { obs_rs_sys::obs_frontend_add_scene_collection(cstr.as_ptr()) })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Consumes a `**char` returned from a `bfree`-managed OBS API into a
/// `Vec<String>`. The whole allocation is freed via `bfree` per the
/// `obs_frontend_get_*_names` contract: the array is a single block.
///
/// # Safety
///
/// `ptr` must be either null or a pointer obtained from the matching
/// `obs_frontend_get_*` API.
pub(crate) unsafe fn take_string_array(ptr: *mut *mut c_char) -> Vec<String> {
    if ptr.is_null() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut p = ptr;
    unsafe {
        while !(*p).is_null() {
            let s = CStr::from_ptr(*p).to_string_lossy().into_owned();
            out.push(s);
            p = p.add(1);
        }
        bfree(ptr as *mut c_void);
    }
    out
}

/// Calls `fill` to populate an `obs_frontend_source_list`, claims each
/// source pointer (transferring the existing refcount), and frees the
/// darray storage. Equivalent to the inline `obs_frontend_source_list_free`
/// helper, but the source refs are returned to the caller as owned
/// [`SourceRef`]s rather than released.
///
/// # Safety
///
/// `fill` must be a valid frontend function that populates the passed
/// `obs_frontend_source_list` with refcounted source pointers.
pub(crate) unsafe fn drain_source_list<F>(fill: F) -> Vec<SourceRef>
where
    F: FnOnce(*mut obs_frontend_source_list),
{
    let mut list = obs_frontend_source_list::default();
    fill(&mut list);
    let inner = unsafe { list.sources.__bindgen_anon_1 };
    let mut out = Vec::with_capacity(inner.num);
    for i in 0..inner.num {
        let ptr: *mut obs_source_t = unsafe { *inner.array.add(i) };
        if let Some(r) = unsafe { SourceRef::from_raw_unchecked(ptr) } {
            out.push(r);
        }
    }
    if !inner.array.is_null() {
        unsafe { bfree(inner.array as *mut c_void) };
    }
    out
}
