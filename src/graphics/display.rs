use obs_sys_rs::{
    obs_display_add_draw_callback, obs_display_destroy, obs_display_enabled,
    obs_display_remove_draw_callback, obs_display_resize, obs_display_set_background_color,
    obs_display_set_enabled, obs_display_size, obs_display_t, obs_render_main_texture,
};

use super::GraphicsColorFormat;

/// An sRGB color with 8-bit per-channel components.
///
/// Built-in constants ([`Color::BLACK`], [`Color::WHITE`], [`Color::RED`],
/// [`Color::GREEN`], [`Color::BLUE`]) cover the common cases. Conversion
/// to and from linear-space sRGB is provided for callers that need to
/// blend colors in the OBS render pipeline's working space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel.
    pub a: u8,
}

fn srgb_nonlinear_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb_linear_to_nonlinear(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
impl Color {
    pub const BLACK: Color = Color::new(0, 0, 0, 255);
    pub const WHITE: Color = Color::new(255, 255, 255, 255);
    pub const RED: Color = Color::new(255, 0, 0, 255);
    pub const GREEN: Color = Color::new(0, 255, 0, 255);
    pub const BLUE: Color = Color::new(0, 0, 255, 255);

    /// Constructs a color from raw 8-bit per-channel components.
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// Encodes the color as a packed 32-bit value in the requested
    /// pixel format.
    ///
    /// # Panics
    ///
    /// Panics if `format` is anything other than
    /// [`GraphicsColorFormat::RGBA`] or [`GraphicsColorFormat::BGRA`].
    pub fn as_format(self, format: GraphicsColorFormat) -> u32 {
        match format {
            GraphicsColorFormat::RGBA => self.as_rgba(),
            GraphicsColorFormat::BGRA => self.as_bgra(),
            _ => unimplemented!("unsupported color format"),
        }
    }

    /// Encodes the color as a packed RGBA `u32`.
    pub fn as_rgba(self) -> u32 {
        u32::from_ne_bytes([self.r, self.g, self.b, self.a])
    }

    /// Encodes the color as a packed BGRA `u32`.
    pub fn as_bgra(self) -> u32 {
        u32::from_ne_bytes([self.b, self.g, self.r, self.a])
    }

    /// Converts the color from sRGB nonlinear to sRGB linear space.
    ///
    /// Mirrors libobs's `gs_float3_srgb_nonlinear_to_linear`.
    pub fn srgb_nonlinear_to_linear(self) -> Self {
        let r = srgb_nonlinear_to_linear(self.r as f32 / 255.0);
        let g = srgb_nonlinear_to_linear(self.g as f32 / 255.0);
        let b = srgb_nonlinear_to_linear(self.b as f32 / 255.0);
        Color {
            r: (r * 255.0) as u8,
            g: (g * 255.0) as u8,
            b: (b * 255.0) as u8,
            a: self.a,
        }
    }

    /// Converts the color from sRGB linear to sRGB nonlinear space.
    pub fn srgb_linear_to_nonlinear(self) -> Self {
        let r = srgb_linear_to_nonlinear(self.r as f32 / 255.0);
        let g = srgb_linear_to_nonlinear(self.g as f32 / 255.0);
        let b = srgb_linear_to_nonlinear(self.b as f32 / 255.0);
        Color {
            r: (r * 255.0) as u8,
            g: (g * 255.0) as u8,
            b: (b * 255.0) as u8,
            a: self.a,
        }
    }
}

/// A handle to an OBS display (`obs_display_t`).
///
/// `DisplayRef` owns the underlying display and destroys it on drop.
/// The handle is not reference-counted, so [`Clone`] is not implemented;
/// wrap in an [`Arc`](std::sync::Arc) if shared ownership is required.
pub struct DisplayRef {
    inner: *mut obs_display_t,
}

impl_ptr_wrapper!(@ptr: inner, DisplayRef, obs_display_t, @identity, obs_display_destroy);

impl DisplayRef {
    /// Returns whether the display is currently enabled (rendering).
    pub fn enabled(&self) -> bool {
        unsafe { obs_display_enabled(self.inner) }
    }

    /// Enables or disables the display.
    pub fn set_enabled(&self, enabled: bool) {
        unsafe { obs_display_set_enabled(self.inner, enabled) }
    }

    /// Returns the display's `(width, height)` in pixels.
    pub fn size(&self) -> (u32, u32) {
        let mut cx = 0;
        let mut cy = 0;
        unsafe { obs_display_size(self.inner, &mut cx, &mut cy) }
        (cx, cy)
    }

    /// Resizes the display.
    pub fn set_size(&self, cx: u32, cy: u32) {
        unsafe { obs_display_resize(self.inner, cx, cy) }
    }

    /// Sets the display's clear color.
    pub fn set_background_color(&self, color: Color) {
        unsafe { obs_display_set_background_color(self.inner, color.as_rgba()) }
    }

    /// Registers a draw callback on the display.
    ///
    /// Returns a [`DrawCallbackId`] that removes the callback on drop.
    /// Use [`DrawCallbackId::forever`] to leak the registration if the
    /// callback should fire for the rest of the display's lifetime.
    pub fn add_draw_callback<S: DrawCallback>(&self, callback: S) -> DrawCallbackId<'_, S> {
        let data = Box::into_raw(Box::new(callback));
        // force the pointer to be a function pointer, since it is not garantueed to be the same pointer
        // for different instance of generic functions
        // see https://users.rust-lang.org/t/generic-functions-and-their-pointer-uniquness/36989
        // clippy: #[deny(clippy::fn_address_comparisons)]
        let callback: unsafe extern "C" fn(*mut std::ffi::c_void, u32, u32) = draw_callback::<S>;
        unsafe {
            obs_display_add_draw_callback(self.inner, Some(callback), data as *mut std::ffi::c_void)
        }
        DrawCallbackId::new(data, callback as *const _, self)
    }

    /// Removes a previously-registered draw callback and returns the
    /// owned callback value.
    pub fn remove_draw_callback<S: DrawCallback>(&self, data: DrawCallbackId<S>) -> S {
        data.take(self)
    }
}

/// A [`DrawCallback`] that renders the OBS main composition texture.
///
/// Useful as a drop-in callback when the only thing the display should
/// draw is the program output.
pub struct RenderMainTexture;

impl DrawCallback for RenderMainTexture {
    fn draw(&self, _cx: u32, _cy: u32) {
        unsafe { obs_render_main_texture() }
    }
}

/// A type that can be invoked as a [`DisplayRef`] draw callback.
pub trait DrawCallback {
    /// Renders into the active display.
    ///
    /// `cx` and `cy` are the display's current width and height in
    /// pixels.
    fn draw(&self, cx: u32, cy: u32);
}

/// FFI thunk that adapts a [`DrawCallback`] into the
/// `obs_display_add_draw_callback` calling convention.
///
/// # Safety
///
/// Called only by OBS with a `data` pointer obtained from
/// [`DrawCallbackId::new`]; user code should not invoke it directly.
pub unsafe extern "C" fn draw_callback<S: DrawCallback>(
    data: *mut std::ffi::c_void,
    cx: u32,
    cy: u32,
) {
    let callback = &*(data as *const S);
    callback.draw(cx, cy);
}

/// RAII handle returned from [`DisplayRef::add_draw_callback`].
///
/// Dropping the handle removes the callback from the display and frees
/// the boxed callback value. Use [`DrawCallbackId::forever`] to keep
/// the registration alive for the rest of the display's lifetime.
pub struct DrawCallbackId<'a, S> {
    data: *mut S,
    callback: *const std::ffi::c_void,
    display: *mut obs_display_t,
    _marker: std::marker::PhantomData<&'a S>,
}

impl<'a, S> DrawCallbackId<'a, S> {
    /// Constructs a new handle binding `data` and `callback` to
    /// `display`. Used internally by [`DisplayRef::add_draw_callback`].
    pub fn new(data: *mut S, callback: *const std::ffi::c_void, display: &'a DisplayRef) -> Self {
        DrawCallbackId {
            data,
            callback,
            display: display.inner,
            _marker: std::marker::PhantomData,
        }
    }

    /// Removes the callback and returns its owned value.
    ///
    /// # Panics
    ///
    /// Panics if `display` is not the same display the callback was
    /// registered on.
    pub fn take(self, display: &DisplayRef) -> S {
        assert_eq!(self.display, display.inner);
        let ptr = self.data;
        unsafe {
            obs_display_add_draw_callback(
                self.display,
                Some(std::mem::transmute::<
                    *const std::ffi::c_void,
                    unsafe extern "C" fn(*mut std::ffi::c_void, u32, u32),
                >(self.callback)),
                ptr as *mut std::ffi::c_void,
            )
        }
        std::mem::forget(self);
        unsafe { *Box::from_raw(ptr) }
    }

    /// Leaks the registration so that it survives until the display is
    /// destroyed.
    ///
    /// The callback continues to run for the lifetime of the underlying
    /// display; once the display is destroyed, libobs releases the
    /// registration and the callback's Box is dropped.
    pub fn forever(self) {
        std::mem::forget(self);
    }
}

impl<'a, S> Drop for DrawCallbackId<'a, S> {
    fn drop(&mut self) {
        unsafe {
            // we don't check validity of the display here
            obs_display_remove_draw_callback(
                self.display,
                Some(std::mem::transmute::<
                    *const std::ffi::c_void,
                    unsafe extern "C" fn(*mut std::ffi::c_void, u32, u32),
                >(self.callback)),
                self.data as *mut std::ffi::c_void,
            );
            drop(Box::from_raw(self.data));
        }
    }
}
