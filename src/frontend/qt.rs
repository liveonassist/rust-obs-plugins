//! Type-safe wrappers around Qt-pointer ownership transfer.
//!
//! The OBS frontend API exchanges raw `void *` pointers for Qt objects in
//! several places. The C contract distinguishes two cases:
//!
//! - **Ownership transfer** (caller → OBS): `obs_frontend_add_dock_by_id` and
//!   `obs_frontend_add_custom_qdock`. OBS reparents the widget into its own
//!   widget tree and owns it for the rest of the process. The caller must
//!   ensure no Rust value will subsequently `delete` the widget.
//! - **Borrow** (OBS → caller): `obs_frontend_get_main_window`,
//!   `obs_frontend_add_tools_menu_qaction`, etc. OBS keeps ownership; the
//!   caller may read/mutate but must not delete.
//!
//! Mixing these up is an FFI hazard: a `cxx::UniquePtr<T>` whose contents
//! were transferred to OBS will, on drop, call `delete` on a widget OBS
//! still references — use-after-free. This module encodes both contracts
//! at the type level via [`OwnedQtPtr`] and [`BorrowedQtPtr`].
//!
//! # Choosing a constructor for [`OwnedQtPtr`]
//!
//! - [`OwnedQtPtr::from_smart_ptr`] — recommended path. Pass the raw pointer
//!   *and* the smart pointer holding it; the call `mem::forget`s the latter
//!   so its destructor never runs.
//! - [`OwnedQtPtr::from_raw`] — escape hatch for callers who already manage
//!   the widget's lifetime themselves (e.g. a hand-written C++ shim that
//!   keeps the widget alive in a static).
//!
//! # Example: adding a custom dock from a `cxx-qt` widget
//!
//! ```ignore
//! // Plugin-side cxx-qt bridge produces a `cxx::UniquePtr<MyDock>` whose
//! // C++ type inherits QWidget.
//! let dock: cxx::UniquePtr<MyDock> = my_dock::create();
//!
//! // Pull out the raw QWidget* before transferring ownership. The exact
//! // accessor depends on your bridge — `as_ref().unwrap() as *const _ as
//! // *mut _` is typical.
//! let raw_widget = dock.as_ref().unwrap() as *const _ as *mut c_void;
//!
//! // Construct the transfer wrapper. `from_smart_ptr` mem::forgets `dock`,
//! // so `dock`'s destructor will not delete the widget OBS now owns.
//! let owned = unsafe { OwnedQtPtr::from_smart_ptr(raw_widget, dock) };
//!
//! // Hand it to OBS — note the call is no longer `unsafe`.
//! obs_rs::frontend::add_dock_by_id("plugin-dock", "Plugin Dock", owned)?;
//! ```

use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr::NonNull;

/// A non-null Qt object pointer whose ownership is being moved into OBS.
///
/// Construct via [`from_smart_ptr`](Self::from_smart_ptr) (preferred) or
/// [`from_raw`](Self::from_raw). The wrapper is `!Send` and `!Sync` because
/// Qt objects can only be touched from the Qt UI thread.
///
/// `OwnedQtPtr` deliberately has no `Drop` impl — if you build one but
/// never hand it to OBS, the underlying widget leaks. That's a small price
/// for ruling out the much worse failure mode of double-free.
#[must_use = "construct this only when about to transfer ownership to OBS; \
              dropping it without handing it off leaks the widget"]
pub struct OwnedQtPtr {
    raw: NonNull<c_void>,
    _not_send: PhantomData<*mut ()>,
}

impl OwnedQtPtr {
    /// Take ownership of a Qt object held by a smart pointer, calling
    /// `std::mem::forget` on the smart pointer.
    ///
    /// `widget_ptr` must be the `void *`-cast Qt-object pointer reachable
    /// through `smart_ptr`; `smart_ptr` is consumed and forgotten so that
    /// its `Drop` will not run. After this call, no Rust value owns the
    /// widget — it must be handed to OBS via [`add_dock_by_id`] /
    /// [`add_custom_qdock`] or it leaks.
    ///
    /// This is the recommended path for any Qt widget allocated via
    /// `cxx-qt` (`cxx::UniquePtr<T>`) or a Rust-side `Box`.
    ///
    /// # Safety
    ///
    /// Caller must ensure all of the following:
    /// - `widget_ptr` is non-null and points at a valid Qt object of the
    ///   type expected by the OBS API call you are about to make
    ///   (`QWidget*` for [`add_dock_by_id`], `QDockWidget*` for
    ///   [`add_custom_qdock`]).
    /// - `widget_ptr` was reachable through `smart_ptr` (i.e. the smart
    ///   pointer was the unique Rust-side owner).
    /// - Nothing else in the process — no other Rust smart pointer, no
    ///   other C++ owner — will subsequently `delete` the widget.
    ///
    /// [`add_dock_by_id`]: super::add_dock_by_id
    /// [`add_custom_qdock`]: super::add_custom_qdock
    pub unsafe fn from_smart_ptr<P>(widget_ptr: *mut c_void, smart_ptr: P) -> Self {
        std::mem::forget(smart_ptr);
        unsafe { Self::from_raw(widget_ptr) }
    }

    /// Wrap a raw pointer when you manage the widget's lifetime yourself.
    ///
    /// Use this when the widget was allocated by a hand-written C++ shim
    /// or some other path where there's no smart pointer to consume.
    ///
    /// # Safety
    ///
    /// Same guarantees as [`from_smart_ptr`](Self::from_smart_ptr) on
    /// `widget_ptr`. The wrapper does not run any destructor — if no Rust
    /// or C++ owner is left, the widget leaks.
    pub unsafe fn from_raw(widget_ptr: *mut c_void) -> Self {
        Self {
            raw: NonNull::new(widget_ptr).expect("OwnedQtPtr: null pointer"),
            _not_send: PhantomData,
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}

/// A borrowed Qt object pointer owned by OBS.
///
/// Returned from getters like [`main_window`](super::main_window) and
/// [`add_tools_menu_qaction`](super::add_tools_menu_qaction). OBS retains
/// ownership; the caller may read/mutate via their own Qt binding (e.g. a
/// `cxx-qt` bridge that takes a `*mut c_void`) but must never `delete`
/// the underlying object.
///
/// The wrapper has no `Drop` impl, so it cannot accidentally trigger a
/// destructor. The lifetime parameter ties validity to the borrowing
/// scope; in practice OBS-owned pointers are valid for the entire process,
/// so the getters return `BorrowedQtPtr<'static>`.
///
/// `!Send` / `!Sync` — Qt objects are UI-thread-only.
pub struct BorrowedQtPtr<'a> {
    raw: NonNull<c_void>,
    _lifetime: PhantomData<&'a ()>,
    _not_send: PhantomData<*mut ()>,
}

impl<'a> BorrowedQtPtr<'a> {
    /// Wrap a raw pointer obtained from a frontend API.
    ///
    /// # Safety
    ///
    /// `raw` must be either null (caller handles via [`from_raw`'s] return
    /// of `None`) or a valid Qt-object pointer that lives at least as long
    /// as `'a`. Caller is responsible for thread-affinity (Qt UI thread).
    ///
    /// [`from_raw`'s]: Self::from_raw
    pub(crate) unsafe fn from_raw(raw: *mut c_void) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self {
            raw,
            _lifetime: PhantomData,
            _not_send: PhantomData,
        })
    }

    /// Returns the raw `*mut c_void` so it can be cast to a Qt type by
    /// the caller's Qt binding.
    pub fn as_ptr(&self) -> *mut c_void {
        self.raw.as_ptr()
    }
}
