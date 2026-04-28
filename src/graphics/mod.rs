pub mod display;

use crate::{Error, Result, native_enum};
use core::convert::TryFrom;
use core::ptr::null_mut;
use obs_sys_rs::{
    GS_DYNAMIC, gs_address_mode, gs_address_mode_GS_ADDRESS_BORDER,
    gs_address_mode_GS_ADDRESS_CLAMP, gs_address_mode_GS_ADDRESS_MIRROR,
    gs_address_mode_GS_ADDRESS_MIRRORONCE, gs_address_mode_GS_ADDRESS_WRAP, gs_color_format,
    gs_color_format_GS_A8, gs_color_format_GS_BGRA, gs_color_format_GS_BGRX,
    gs_color_format_GS_DXT1, gs_color_format_GS_DXT3, gs_color_format_GS_DXT5,
    gs_color_format_GS_R8, gs_color_format_GS_R8G8, gs_color_format_GS_R10G10B10A2,
    gs_color_format_GS_R16, gs_color_format_GS_R16F, gs_color_format_GS_R32F,
    gs_color_format_GS_RG16F, gs_color_format_GS_RG32F, gs_color_format_GS_RGBA,
    gs_color_format_GS_RGBA16, gs_color_format_GS_RGBA16F, gs_color_format_GS_RGBA32F,
    gs_color_format_GS_UNKNOWN, gs_effect_create, gs_effect_destroy, gs_effect_get_param_by_name,
    gs_effect_get_param_info, gs_effect_param_info, gs_effect_set_next_sampler,
    gs_effect_set_texture, gs_effect_set_vec2, gs_effect_t, gs_eparam_t, gs_sample_filter,
    gs_sample_filter_GS_FILTER_ANISOTROPIC, gs_sample_filter_GS_FILTER_LINEAR,
    gs_sample_filter_GS_FILTER_MIN_LINEAR_MAG_MIP_POINT,
    gs_sample_filter_GS_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR,
    gs_sample_filter_GS_FILTER_MIN_MAG_LINEAR_MIP_POINT,
    gs_sample_filter_GS_FILTER_MIN_MAG_POINT_MIP_LINEAR,
    gs_sample_filter_GS_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT,
    gs_sample_filter_GS_FILTER_MIN_POINT_MAG_MIP_LINEAR, gs_sample_filter_GS_FILTER_POINT,
    gs_sampler_info, gs_samplerstate_create, gs_samplerstate_destroy, gs_samplerstate_t,
    gs_shader_param_type, gs_shader_param_type_GS_SHADER_PARAM_BOOL,
    gs_shader_param_type_GS_SHADER_PARAM_FLOAT, gs_shader_param_type_GS_SHADER_PARAM_INT,
    gs_shader_param_type_GS_SHADER_PARAM_INT2, gs_shader_param_type_GS_SHADER_PARAM_INT3,
    gs_shader_param_type_GS_SHADER_PARAM_INT4, gs_shader_param_type_GS_SHADER_PARAM_MATRIX4X4,
    gs_shader_param_type_GS_SHADER_PARAM_STRING, gs_shader_param_type_GS_SHADER_PARAM_TEXTURE,
    gs_shader_param_type_GS_SHADER_PARAM_UNKNOWN, gs_shader_param_type_GS_SHADER_PARAM_VEC2,
    gs_shader_param_type_GS_SHADER_PARAM_VEC3, gs_shader_param_type_GS_SHADER_PARAM_VEC4,
    gs_texture_create, gs_texture_destroy, gs_texture_get_height, gs_texture_get_width,
    gs_texture_map, gs_texture_set_image, gs_texture_t, gs_texture_unmap, obs_allow_direct_render,
    obs_allow_direct_render_OBS_ALLOW_DIRECT_RENDERING,
    obs_allow_direct_render_OBS_NO_DIRECT_RENDERING, obs_enter_graphics, obs_leave_graphics,
    obs_source_draw, vec2, vec3, vec4,
};
use paste::item;
use std::{
    ffi::{CStr, CString},
    ptr,
};
use std::{os::raw::c_int, slice};

/// Guard to guarantee that we exit graphics context properly.
/// This does not prevent one from calling APIs that are not supposed to be
/// called outside of the context.
struct GraphicsGuard;

impl GraphicsGuard {
    fn enter() -> Self {
        unsafe {
            obs_enter_graphics();
        }
        Self
    }

    pub fn with_enter<T, F: FnOnce() -> T>(f: F) -> T {
        let _g = Self::enter();
        f()
    }
}

impl Drop for GraphicsGuard {
    fn drop(&mut self) {
        unsafe {
            obs_leave_graphics();
        }
    }
}

native_enum!(ShaderParamType, gs_shader_param_type {
    Unknown => GS_SHADER_PARAM_UNKNOWN,
    Bool => GS_SHADER_PARAM_BOOL,
    Float => GS_SHADER_PARAM_FLOAT,
    Int => GS_SHADER_PARAM_INT,
    String => GS_SHADER_PARAM_STRING,
    Vec2 => GS_SHADER_PARAM_VEC2,
    Vec3 => GS_SHADER_PARAM_VEC3,
    Vec4 => GS_SHADER_PARAM_VEC4,
    Int2 => GS_SHADER_PARAM_INT2,
    Int3 => GS_SHADER_PARAM_INT3,
    Int4 => GS_SHADER_PARAM_INT4,
    Mat4 => GS_SHADER_PARAM_MATRIX4X4,
    Texture => GS_SHADER_PARAM_TEXTURE,
});

/// A compiled graphics effect (`gs_effect_t`).
///
/// Effects encapsulate the shaders and parameters used to render OBS
/// sources and filters. Construct with
/// [`GraphicsEffect::from_effect_string`]; access individual parameters
/// via [`GraphicsEffect::get_effect_param_by_name`].
pub struct GraphicsEffect {
    raw: *mut gs_effect_t,
}

impl GraphicsEffect {
    /// Compiles an effect from source code.
    ///
    /// `value` is the effect's source text; `name` is the identifier
    /// used in compiler diagnostics. Returns `None` if compilation
    /// fails.
    pub fn from_effect_string(value: &CStr, name: &CStr) -> Option<Self> {
        let raw = GraphicsGuard::with_enter(|| unsafe {
            gs_effect_create(value.as_ptr(), name.as_ptr(), std::ptr::null_mut())
        });
        if raw.is_null() {
            None
        } else {
            Some(Self { raw })
        }
    }

    /// Looks up a parameter by name and converts it to the target
    /// strongly-typed wrapper.
    ///
    /// Returns `None` if the parameter does not exist or its declared
    /// shader type does not match `T`.
    pub fn get_effect_param_by_name<T: TryFrom<GraphicsEffectParam>>(
        &mut self,
        name: &CStr,
    ) -> Option<T> {
        unsafe {
            let pointer = gs_effect_get_param_by_name(self.raw, name.as_ptr());
            if !pointer.is_null() {
                T::try_from(GraphicsEffectParam::from_raw(pointer)).ok()
            } else {
                None
            }
        }
    }

    /// Returns the underlying `gs_effect_t*`.
    ///
    /// # Safety
    ///
    /// Mutating effect state behind libobs's back can leave the graphics
    /// pipeline in an inconsistent state. The pointer must not be released
    /// by the caller.
    pub unsafe fn as_ptr(&self) -> *mut gs_effect_t {
        self.raw
    }
}

impl Drop for GraphicsEffect {
    fn drop(&mut self) {
        GraphicsGuard::with_enter(|| unsafe {
            gs_effect_destroy(self.raw);
        });
    }
}

/// Errors returned when narrowing a [`GraphicsEffectParam`] to a
/// strongly-typed wrapper such as [`GraphicsEffectVec2Param`].
pub enum GraphicsEffectParamConversionError {
    /// The parameter exists but its declared shader type does not match
    /// the wrapper being requested.
    InvalidType,
}

/// A handle to a single parameter on a [`GraphicsEffect`].
///
/// `GraphicsEffectParam` carries the parameter's name and declared
/// shader type. Convert it to a typed wrapper (e.g.
/// [`GraphicsEffectVec2Param`], [`GraphicsEffectTextureParam`]) before
/// reading or writing values.
pub struct GraphicsEffectParam {
    raw: *mut gs_eparam_t,
    name: String,
    shader_type: ShaderParamType,
}

impl GraphicsEffectParam {
    /// Wraps a raw `gs_eparam_t*`.
    ///
    /// # Safety
    ///
    /// `raw` must point at a parameter that lives at least as long as
    /// the resulting `GraphicsEffectParam`, and must not be mutated
    /// concurrently from outside the wrapper.
    pub unsafe fn from_raw(raw: *mut gs_eparam_t) -> Self {
        let mut info = gs_effect_param_info::default();
        gs_effect_get_param_info(raw, &mut info);

        let shader_type = ShaderParamType::from_raw(info.type_).unwrap();
        let name = CString::from(CStr::from_ptr(info.name))
            .into_string()
            .unwrap_or_else(|_| String::from("{unknown-param-name}"));

        Self {
            raw,
            shader_type,
            name,
        }
    }

    /// Returns the parameter's declared name.
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

macro_rules! impl_graphics_effects {
    ($($t:ident)*) => {
        $(
            item! {
                pub struct [<GraphicsEffect $t Param>] {
                    effect: GraphicsEffectParam,
                }

                impl TryFrom<GraphicsEffectParam> for [<GraphicsEffect $t Param>] {
                    type Error = Error;

                    fn try_from(effect: GraphicsEffectParam) -> Result<Self> {
                        match effect.shader_type {
                            ShaderParamType::[<$t>] => Ok([<GraphicsEffect $t Param>] { effect }),
                            _ => Err(Error::EnumOutOfRange("ShaderParamType", effect.shader_type as _)),
                        }
                    }
                }
            }
        )*
    };
}

impl_graphics_effects! {
    Vec2
    Texture
}

impl GraphicsEffectVec2Param {
    pub fn set_vec2(&mut self, _context: &GraphicsEffectContext, value: &Vec2) {
        unsafe {
            gs_effect_set_vec2(self.effect.raw, &value.raw);
        }
    }
}

impl GraphicsEffectTextureParam {
    pub fn set_next_sampler(
        &mut self,
        _context: &GraphicsEffectContext,
        value: &mut GraphicsSamplerState,
    ) {
        unsafe {
            gs_effect_set_next_sampler(self.effect.raw, value.raw);
        }
    }

    pub fn set_texture(&mut self, _context: &GraphicsEffectContext, value: &GraphicsTexture) {
        unsafe {
            gs_effect_set_texture(self.effect.raw, value.raw);
        }
    }
}

native_enum!(GraphicsAddressMode, gs_address_mode {
    Clamp => GS_ADDRESS_CLAMP,
    Wrap => GS_ADDRESS_WRAP,
    Mirror => GS_ADDRESS_MIRROR,
    Border => GS_ADDRESS_BORDER,
    MirrorOnce => GS_ADDRESS_MIRRORONCE,
});

native_enum!(GraphicsSampleFilter, gs_sample_filter {
    Point => GS_FILTER_POINT,
    Linear => GS_FILTER_LINEAR,
    Anisotropic => GS_FILTER_ANISOTROPIC,
    MinMagPointMipLinear => GS_FILTER_MIN_MAG_POINT_MIP_LINEAR,
    MinPointMagLinearMipPoint => GS_FILTER_MIN_POINT_MAG_LINEAR_MIP_POINT,
    MinPointMagMipLinear => GS_FILTER_MIN_POINT_MAG_MIP_LINEAR,
    MinLinearMapMipPoint => GS_FILTER_MIN_LINEAR_MAG_MIP_POINT,
    MinLinearMagPointMipLinear => GS_FILTER_MIN_LINEAR_MAG_POINT_MIP_LINEAR,
    MinMagLinearMipPoint => GS_FILTER_MIN_MAG_LINEAR_MIP_POINT,
});

/// Builder for a [`GraphicsSamplerState`].
///
/// Configures wrap modes for each axis and the texture filter. Convert
/// to a usable sampler with `Into::into`.
pub struct GraphicsSamplerInfo {
    info: gs_sampler_info,
}

impl GraphicsSamplerInfo {
    /// Creates a new sampler description with clamp wrapping on every
    /// axis and point filtering.
    pub fn new() -> Self {
        Self {
            info: gs_sampler_info {
                address_u: GraphicsAddressMode::Clamp.as_raw(),
                address_v: GraphicsAddressMode::Clamp.as_raw(),
                address_w: GraphicsAddressMode::Clamp.as_raw(),
                max_anisotropy: 0,
                border_color: 0,
                filter: GraphicsSampleFilter::Point.as_raw(),
            },
        }
    }

    /// Sets the wrap mode for the U axis.
    pub fn with_address_u(mut self, mode: GraphicsAddressMode) -> Self {
        self.info.address_u = mode.as_raw();
        self
    }

    /// Sets the wrap mode for the V axis.
    pub fn with_address_v(mut self, mode: GraphicsAddressMode) -> Self {
        self.info.address_v = mode.as_raw();
        self
    }

    /// Sets the wrap mode for the W axis.
    pub fn with_address_w(mut self, mode: GraphicsAddressMode) -> Self {
        self.info.address_w = mode.as_raw();
        self
    }

    /// Sets the texture filter mode.
    pub fn with_filter(mut self, mode: GraphicsSampleFilter) -> Self {
        self.info.filter = mode.as_raw();
        self
    }
}

impl Default for GraphicsSamplerInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// A compiled sampler state (`gs_samplerstate_t`).
///
/// Construct from a [`GraphicsSamplerInfo`] via `Into::into`. The
/// destructor releases the underlying object.
pub struct GraphicsSamplerState {
    raw: *mut gs_samplerstate_t,
}

impl From<GraphicsSamplerInfo> for GraphicsSamplerState {
    fn from(info: GraphicsSamplerInfo) -> GraphicsSamplerState {
        let raw = GraphicsGuard::with_enter(|| unsafe { gs_samplerstate_create(&info.info) });
        GraphicsSamplerState { raw }
    }
}

impl Drop for GraphicsSamplerState {
    fn drop(&mut self) {
        GraphicsGuard::with_enter(|| unsafe {
            gs_samplerstate_destroy(self.raw);
        });
    }
}

/// A capability token signalling that the calling thread is currently
/// inside an active effect-rendering context.
///
/// Methods that require an active graphics context borrow a
/// `GraphicsEffectContext` rather than asserting it implicitly. The
/// crate constructs it at the appropriate boundary; user code never
/// produces one directly.
pub struct GraphicsEffectContext {}

impl GraphicsEffectContext {
    /// Constructs a fresh `GraphicsEffectContext`.
    ///
    /// # Safety
    ///
    /// Only valid while the calling thread is inside an active OBS
    /// effect-rendering context (e.g. between
    /// `obs_source_process_filter_begin` and `_end`).
    pub(crate) unsafe fn new() -> Self {
        Self {}
    }
}

native_enum!(GraphicsColorFormat, gs_color_format {
    UNKNOWN => GS_UNKNOWN,
    A8 => GS_A8,
    R8 => GS_R8,
    RGBA => GS_RGBA,
    BGRX => GS_BGRX,
    BGRA => GS_BGRA,
    R10G10B10A2 => GS_R10G10B10A2,
    RGBA16 => GS_RGBA16,
    R16 => GS_R16,
    RGBA16F => GS_RGBA16F,
    RGBA32F => GS_RGBA32F,
    RG16F => GS_RG16F,
    RG32F => GS_RG32F,
    R16F => GS_R16F,
    R32F => GS_R32F,
    DXT1 => GS_DXT1,
    DXT3 => GS_DXT3,
    DXT5 => GS_DXT5,
    R8G8 => GS_R8G8,
});

native_enum!(GraphicsAllowDirectRendering, obs_allow_direct_render {
    NoDirectRendering => OBS_NO_DIRECT_RENDERING,
    AllowDirectRendering => OBS_ALLOW_DIRECT_RENDERING,
});

macro_rules! vector_impls {
    ($($rust_name: ident, $name:ident => $($component:ident)*,)*) => (
        $(
        #[derive(Clone)]
        pub struct $rust_name {
            raw: $name,
        }

        impl $rust_name {
            pub fn new($( $component: f32, )*) -> Self {
                let mut v = Self {
                    raw: $name::default(),
                };
                v.set($($component,)*);
                v
            }

            #[inline]
            pub fn zero(&mut self) {
                $(
                    self.raw.__bindgen_anon_1.__bindgen_anon_1.$component = 0.;
                )*
            }

            #[inline]
            pub fn copy(&mut self, input: &$rust_name) {
                self.set($(input.$component(),)*);
            }

            #[inline]
            pub fn add(&mut self, input: &$rust_name) {
                self.set($(self.$component() + input.$component(),)*);
            }

            #[inline]
            pub fn sub(&mut self, input: &$rust_name) {
                self.set($(self.$component() - input.$component(),)*);
            }

            #[inline]
            pub fn mul(&mut self, input: &$rust_name) {
                self.set($(self.$component() * input.$component(),)*);
            }

            #[inline]
            pub fn div(&mut self, input: &$rust_name) {
                self.set($(self.$component() / input.$component(),)*);
            }

            #[inline]
            pub fn addf(&mut self, input: f32) {
                self.set($(self.$component() + input,)*);
            }

            #[inline]
            pub fn subf(&mut self, input: f32) {
                self.set($(self.$component() - input,)*);
            }

            #[inline]
            pub fn mulf(&mut self, input: f32) {
                self.set($(self.$component() * input,)*);
            }

            #[inline]
            pub fn divf(&mut self, input: f32) {
                self.set($(self.$component() / input,)*);
            }

            #[inline]
            pub fn neg(&mut self) {
                self.set($(-self.$component(),)*);
            }

            #[inline]
            pub fn dot(&mut self, input: &$rust_name) -> f32 {
                $(
                    self.$component() * input.$component() +
                )* 0.
            }

            #[inline]
            pub fn len(&mut self) -> f32 {
                ($( self.$component() * self.$component() + )* 0.).sqrt()
            }

            #[inline]
            pub fn set(&mut self, $( $component: f32, )*) {
                $(
                    self.raw.__bindgen_anon_1.__bindgen_anon_1.$component = $component;
                )*
            }

            #[inline]
            pub fn min(&mut self, input: &$rust_name) {
                self.set($(self.$component().min(input.$component()),)*);
            }

            #[inline]
            pub fn max(&mut self, input: &$rust_name) {
                self.set($(self.$component().max(input.$component()),)*);
            }

            #[inline]
            pub fn minf(&mut self, input: f32) {
                self.set($(self.$component().min(input),)*);
            }

            #[inline]
            pub fn maxf(&mut self, input: f32) {
                self.set($(self.$component().max(input),)*);
            }

            #[inline]
            pub fn abs(&mut self) {
                self.set($(self.$component().abs(),)*);
            }

            #[inline]
            pub fn ceil(&mut self) {
                self.set($(self.$component().ceil(),)*);
            }

            #[inline]
            pub fn floor(&mut self) {
                self.set($(self.$component().floor(),)*);
            }

            #[inline]
            pub fn close(&mut self, input: &$rust_name, epsilon: f32) -> bool {
                $(
                    (self.$component() - input.$component()).abs() > epsilon &&
                )* true
            }

            $(
                item! {
                    #[inline]
                    pub fn [<$component>](&self) -> f32 {
                        unsafe {
                            self.raw.__bindgen_anon_1.__bindgen_anon_1.$component
                        }
                    }
                }
            )*

            pub fn as_ptr(&mut self) -> *mut $name {
                &mut self.raw
            }
        }

        impl Default for $rust_name {
            fn default() -> Self {
                $(
                    let $component = 0.;
                )*
                Self::new($( $component, )*)
            }
        }
        )*
    );
}

vector_impls! {
    Vec2, vec2 => x y,
    Vec3, vec3 => x y z,
    Vec4, vec4 => x y z w,
}

/// A GPU-resident texture (`gs_texture_t`).
///
/// `GraphicsTexture` owns a libobs-managed texture; the destructor
/// releases the underlying object.
///
/// See the [OBS reference][docs] for the underlying C API.
///
/// [docs]: https://obsproject.com/docs/reference-libobs-graphics-graphics.html#c.gs_texture_t
pub struct GraphicsTexture {
    raw: *mut gs_texture_t,
}

impl GraphicsTexture {
    /// Allocates a new dynamic texture with the given dimensions and
    /// pixel format.
    pub fn new(width: u32, height: u32, format: GraphicsColorFormat) -> Self {
        let raw = GraphicsGuard::with_enter(|| unsafe {
            gs_texture_create(width, height, format.as_raw(), 1, null_mut(), GS_DYNAMIC)
        });
        Self { raw }
    }

    /// Returns the texture height, in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        GraphicsGuard::with_enter(|| unsafe { gs_texture_get_height(self.raw) })
    }

    /// Returns the texture width, in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        GraphicsGuard::with_enter(|| unsafe { gs_texture_get_width(self.raw) })
    }

    /// Uploads CPU-side pixel data into the texture.
    ///
    /// `linesize` is the source stride in bytes. When `invert` is true,
    /// the source rows are uploaded bottom-up.
    pub fn set_image(&mut self, data: &[u8], linesize: u32, invert: bool) {
        GraphicsGuard::with_enter(|| unsafe {
            gs_texture_set_image(self.raw, data.as_ptr(), linesize, invert);
        });
    }

    /// Draws the texture as an OBS source quad at `(x, y)` sized
    /// `(cx, cy)`. When `flip` is true, the result is flipped
    /// vertically.
    pub fn draw(&self, x: c_int, y: c_int, cx: u32, cy: u32, flip: bool) {
        unsafe {
            obs_source_draw(self.raw, x, y, cx, cy, flip);
        }
    }

    /// Maps the texture into CPU-addressable memory for read/write
    /// access.
    ///
    /// Returns an RAII [`MappedTexture`] guard; dropping it unmaps the
    /// texture.
    #[inline]
    pub fn map(&mut self) -> Result<MappedTexture<'_>> {
        MappedTexture::new(self)
    }

    /// Returns the underlying `gs_texture_t*`.
    pub fn as_ptr(&self) -> *mut gs_texture_t {
        self.raw
    }
}

impl Drop for GraphicsTexture {
    fn drop(&mut self) {
        GraphicsGuard::with_enter(|| unsafe {
            gs_texture_destroy(self.raw);
        });
    }
}

/// CPU-side view into a [`GraphicsTexture`] obtained via
/// [`GraphicsTexture::map`].
///
/// Implements [`Deref`](std::ops::Deref) and [`DerefMut`](std::ops::DerefMut)
/// to a byte slice covering the mapped pixels. Dropping the guard
/// unmaps the texture.
pub struct MappedTexture<'tex> {
    tex: &'tex mut GraphicsTexture,
    ptr: *mut u8,
    len: usize,
}

impl<'tex> MappedTexture<'tex> {
    fn new(tex: &'tex mut GraphicsTexture) -> Result<Self> {
        let mut ptr: *mut u8 = ptr::null_mut();
        let mut linesize = 0u32;
        let map_result = GraphicsGuard::with_enter(|| unsafe {
            gs_texture_map(tex.as_ptr(), &mut ptr, &mut linesize)
        });
        if !map_result {
            return Err(Error::ObsError(-1));
        }
        let len = (linesize * tex.height()) as usize;
        Ok(Self { tex, ptr, len })
    }

    /// Returns a raw pointer to the start of the mapped pixel data.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// Returns a raw mutable pointer to the start of the mapped pixel
    /// data.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    /// Returns the texture width, in pixels.
    #[inline]
    pub fn width(&self) -> u32 {
        self.tex.width()
    }

    /// Returns the texture height, in pixels.
    #[inline]
    pub fn height(&self) -> u32 {
        self.tex.height()
    }
}

impl std::ops::Deref for MappedTexture<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.as_ptr(), self.len) }
    }
}

impl std::ops::DerefMut for MappedTexture<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len) }
    }
}

impl std::fmt::Debug for MappedTexture<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&**self, f)
    }
}

impl Drop for MappedTexture<'_> {
    fn drop(&mut self) {
        GraphicsGuard::with_enter(|| unsafe {
            gs_texture_unmap(self.tex.as_ptr());
        });
    }
}
