//! OBS 31+ canvas API.
//!
//! Canvases are independent video pipelines added in OBS 31. The frontend
//! API exposes them via [`canvases`] / [`add_canvas`] / [`remove_canvas`].

use std::ffi::CString;
use std::os::raw::c_void;

use obs_rs_sys::{
    bfree, obs_canvas_get_ref, obs_canvas_release, obs_canvas_t, obs_frontend_add_canvas,
    obs_frontend_canvas_list, obs_frontend_get_canvases, obs_frontend_remove_canvas, obs_video_info,
};

use crate::wrapper::PtrWrapper;

/// Owned reference to an `obs_canvas_t`.
///
/// Drop releases OBS's refcount.
pub struct CanvasRef {
    inner: *mut obs_canvas_t,
}

impl_ptr_wrapper!(
    @ptr: inner,
    CanvasRef,
    obs_canvas_t,
    obs_canvas_get_ref,
    obs_canvas_release
);

/// Every active canvas in OBS Studio.
pub fn canvases() -> Vec<CanvasRef> {
    let mut list = obs_frontend_canvas_list::default();
    unsafe { obs_frontend_get_canvases(&mut list) };
    let inner = unsafe { list.canvases.__bindgen_anon_1 };
    let mut out = Vec::with_capacity(inner.num);
    for i in 0..inner.num {
        let ptr: *mut obs_canvas_t = unsafe { *inner.array.add(i) };
        if let Some(r) = unsafe { CanvasRef::from_raw_unchecked(ptr) } {
            out.push(r);
        }
    }
    if !inner.array.is_null() {
        unsafe { bfree(inner.array as *mut c_void) };
    }
    out
}

/// Creates a new canvas with the given name and video parameters.
///
/// `flags` is a bitmask of `OBS_CANVAS_*` flags from `libobs`. Returns
/// `None` if OBS rejects the request.
///
/// # Safety
///
/// `video_info` must point at a fully-initialized [`obs_video_info`]
/// struct. The struct is read but not retained by OBS.
pub unsafe fn add_canvas(
    name: &str,
    video_info: &mut obs_video_info,
    flags: i32,
) -> Result<Option<CanvasRef>, std::ffi::NulError> {
    let cstr = CString::new(name)?;
    let ptr = unsafe { obs_frontend_add_canvas(cstr.as_ptr(), video_info, flags) };
    Ok(unsafe { CanvasRef::from_raw_unchecked(ptr) })
}

/// Removes the given canvas. Returns `true` if the canvas was found and
/// removed.
pub fn remove_canvas(canvas: &CanvasRef) -> bool {
    unsafe { obs_frontend_remove_canvas(canvas.as_ptr() as *mut _) }
}
