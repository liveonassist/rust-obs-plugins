//! Bindings for authoring custom OBS encoders.
//!
//! An encoder consumes raw video frames or audio samples and produces
//! compressed packets. Plugins register encoders with OBS at module load
//! time; OBS then drives them through the libobs encode pipeline.
//!
//! # Authoring an encoder
//!
//! 1. Define a type that holds the per-instance state of your encoder.
//! 2. Implement [`Encodable`](crate::encoder::traits::Encodable) for it — this is mandatory
//!    and identifies the encoder, names the codec, and constructs the
//!    per-instance state.
//! 3. Implement any of the optional traits
//!    ([`GetNameEncoder`](crate::encoder::traits::GetNameEncoder),
//!    [`EncodeEncoder`](crate::encoder::traits::EncodeEncoder),
//!    [`EncodeTextureEncoder`](crate::encoder::traits::EncodeTextureEncoder),
//!    [`UpdateEncoder`](crate::encoder::traits::UpdateEncoder), …) that map to the libobs
//!    callbacks your encoder needs.
//! 4. Build an [`EncoderInfo`](crate::encoder::EncoderInfo) with
//!    [`EncoderInfoBuilder`](crate::encoder::EncoderInfoBuilder), opting
//!    each optional trait in with the matching `enable_*` method, and pass
//!    it to [`LoadContext::register_encoder`].
//!
//! Every encoder must wire up at least one encode path — call
//! [`EncoderInfoBuilder::enable_encode`](crate::encoder::EncoderInfoBuilder::enable_encode)
//! for software encoders or
//! [`EncoderInfoBuilder::enable_encode_texture`](crate::encoder::EncoderInfoBuilder::enable_encode_texture)
//! for GPU-path encoders. Audio encoders must additionally call
//! [`EncoderInfoBuilder::enable_get_frame_size`](crate::encoder::EncoderInfoBuilder::enable_get_frame_size).
//!
//! # Example
//!
//! ```ignore
//! use std::ffi::CStr;
//! use obs_rs::encoder::*;
//!
//! struct MyH264 { /* per-instance state */ }
//!
//! impl Encodable for MyH264 {
//!     fn get_id() -> &'static CStr { c"my_h264" }
//!     fn get_codec() -> &'static CStr { c"h264" }
//!     fn get_type() -> EncoderType { EncoderType::Video }
//!
//!     fn create(
//!         ctx: &mut CreatableEncoderContext<Self>,
//!         encoder: EncoderRef,
//!     ) -> Result<Self, CreateError> {
//!         Ok(MyH264 { /* … */ })
//!     }
//! }
//!
//! impl GetNameEncoder for MyH264 {
//!     fn get_name() -> &'static CStr { c"My H.264 Encoder" }
//! }
//!
//! impl EncodeEncoder for MyH264 {
//!     fn encode(
//!         &mut self,
//!         frame: &EncoderFrame<'_>,
//!         packet: &mut EncoderPacket<'_>,
//!     ) -> Result<EncodeStatus, EncodeError> {
//!         packet.reset();
//!         packet.set_pts(frame.pts());
//!         // … encode and write payload bytes …
//!         Ok(EncodeStatus::Received)
//!     }
//! }
//!
//! // In `Module::load`:
//! load_context.register_encoder(
//!     load_context
//!         .create_encoder_builder::<MyH264>()
//!         .enable_get_name()
//!         .enable_encode()
//!         .with_caps(EncoderCap::PassTexture | EncoderCap::DynBitrate)
//!         .build(),
//! );
//! ```
//!
//! [`LoadContext::register_encoder`]: crate::module::LoadContext::register_encoder

pub mod context;
mod ffi;
pub mod traits;

use std::marker::PhantomData;

use enumflags2::{BitFlags, bitflags};
#[cfg(any(feature = "obs-31", feature = "obs-32"))]
use obs_sys_rs::OBS_ENCODER_CAP_SCALING;
use obs_sys_rs::{
    OBS_ENCODER_CAP_DEPRECATED, OBS_ENCODER_CAP_DYN_BITRATE, OBS_ENCODER_CAP_INTERNAL,
    OBS_ENCODER_CAP_PASS_TEXTURE, OBS_ENCODER_CAP_ROI, obs_encoder_get_codec,
    obs_encoder_get_height, obs_encoder_get_id, obs_encoder_get_name, obs_encoder_get_ref,
    obs_encoder_get_width, obs_encoder_info, obs_encoder_release, obs_encoder_t, obs_encoder_type,
    obs_encoder_type_OBS_ENCODER_AUDIO, obs_encoder_type_OBS_ENCODER_VIDEO,
};
use paste::item;

use std::ffi::CString;

use crate::media::{audio::AudioRef, video::VideoRef};
use crate::string::cstring_from_ptr;
use crate::wrapper::PtrWrapper;
use crate::{Error, Result};

pub use crate::encoder::traits::*;
pub use context::*;

/// The kind of media an encoder consumes.
///
/// Returned by [`Encodable::get_type`] and used by OBS to route an
/// encoder to the appropriate audio or video pipeline.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EncoderType {
    /// The encoder consumes raw audio samples.
    Audio,
    /// The encoder consumes raw video frames.
    Video,
}

impl EncoderType {
    /// Converts to the underlying libobs enumerant.
    pub fn as_raw(self) -> obs_encoder_type {
        match self {
            EncoderType::Audio => obs_encoder_type_OBS_ENCODER_AUDIO,
            EncoderType::Video => obs_encoder_type_OBS_ENCODER_VIDEO,
        }
    }
}

/// Capability flags advertised by an encoder.
///
/// Compose multiple flags with `|`; the resulting [`BitFlags`] is passed to
/// [`EncoderInfoBuilder::with_caps`]. The set of available flags depends on
/// the targeted libobs version.
//
// Split into per-version enum bodies because `enumflags2`'s `#[bitflags]`
// attribute does not propagate `#[cfg]` on individual variants — the impl
// code it expands references every variant unconditionally. Keeping the cfg
// at item level avoids that.
#[cfg(feature = "obs-30")]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EncoderCap {
    /// Encoder is deprecated and should not be exposed to new users.
    Deprecated = OBS_ENCODER_CAP_DEPRECATED,
    /// Encoder consumes shared textures rather than CPU buffers. Required
    /// for [`EncodeTextureEncoder`].
    PassTexture = OBS_ENCODER_CAP_PASS_TEXTURE,
    /// Encoder supports changing its bitrate at runtime.
    DynBitrate = OBS_ENCODER_CAP_DYN_BITRATE,
    /// Encoder is for libobs-internal use only and should not appear in
    /// user-facing pickers.
    Internal = OBS_ENCODER_CAP_INTERNAL,
    /// Encoder accepts region-of-interest hints.
    Roi = OBS_ENCODER_CAP_ROI,
}

/// Capability flags advertised by an encoder.
///
/// Compose multiple flags with `|`; the resulting [`BitFlags`] is passed to
/// [`EncoderInfoBuilder::with_caps`]. The set of available flags depends on
/// the targeted libobs version.
#[cfg(any(feature = "obs-31", feature = "obs-32"))]
#[bitflags]
#[repr(u32)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EncoderCap {
    /// Encoder is deprecated and should not be exposed to new users.
    Deprecated = OBS_ENCODER_CAP_DEPRECATED,
    /// Encoder consumes shared textures rather than CPU buffers. Required
    /// for [`EncodeTextureEncoder`].
    PassTexture = OBS_ENCODER_CAP_PASS_TEXTURE,
    /// Encoder supports changing its bitrate at runtime.
    DynBitrate = OBS_ENCODER_CAP_DYN_BITRATE,
    /// Encoder is for libobs-internal use only and should not appear in
    /// user-facing pickers.
    Internal = OBS_ENCODER_CAP_INTERNAL,
    /// Encoder accepts region-of-interest hints.
    Roi = OBS_ENCODER_CAP_ROI,
    /// Encoder can scale its input to a different output resolution.
    /// Available on OBS 31+.
    Scaling = OBS_ENCODER_CAP_SCALING,
}

/// A reference-counted handle to a live OBS encoder instance.
///
/// `EncoderRef` is the safe Rust counterpart to libobs's `obs_encoder_t`.
/// Cloning increments the underlying reference count; dropping releases it.
/// The handle exposes read-only inspection of the encoder's identity and
/// dimensions, and lookups for any video or audio output the encoder is
/// bound to.
pub struct EncoderRef {
    inner: *mut obs_encoder_t,
}

impl_ptr_wrapper!(
    @ptr: inner,
    EncoderRef,
    obs_encoder_t,
    obs_encoder_get_ref,
    obs_encoder_release
);

impl EncoderRef {
    /// Returns the user-visible name of the encoder instance, as set by the
    /// caller of `obs_encoder_create`.
    pub fn name(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_encoder_get_name(self.inner)) }
            .ok_or(Error::NulPointer("obs_encoder_get_name"))
    }

    /// Returns the registered identifier for this encoder type
    /// (matches [`Encodable::get_id`]).
    pub fn id(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_encoder_get_id(self.inner)) }
            .ok_or(Error::NulPointer("obs_encoder_get_id"))
    }

    /// Returns the codec name produced by this encoder
    /// (matches [`Encodable::get_codec`]).
    pub fn codec(&self) -> Result<CString> {
        unsafe { cstring_from_ptr(obs_encoder_get_codec(self.inner)) }
            .ok_or(Error::NulPointer("obs_encoder_get_codec"))
    }

    /// Returns the output width in pixels (video encoders only).
    pub fn width(&self) -> u32 {
        unsafe { obs_encoder_get_width(self.inner) }
    }

    /// Returns the output height in pixels (video encoders only).
    pub fn height(&self) -> u32 {
        unsafe { obs_encoder_get_height(self.inner) }
    }

    /// Returns the video output this encoder is bound to, or `None` if it
    /// has not been associated with one. Only meaningful for video encoders.
    pub fn video(&self) -> Option<VideoRef> {
        let ptr = unsafe { obs_sys_rs::obs_encoder_video(self.inner) };
        if ptr.is_null() {
            None
        } else {
            Some(VideoRef::from_raw(ptr))
        }
    }

    /// Returns the audio output this encoder is bound to, or `None` if it
    /// has not been associated with one. Only meaningful for audio encoders.
    pub fn audio(&self) -> Option<AudioRef> {
        let ptr = unsafe { obs_sys_rs::obs_encoder_audio(self.inner) };
        if ptr.is_null() {
            None
        } else {
            Some(AudioRef::from_raw(ptr))
        }
    }
}

/// A fully-configured encoder registration, ready to be handed to OBS.
///
/// Produced by [`EncoderInfoBuilder::build`] and consumed by
/// [`LoadContext::register_encoder`], which keeps the underlying
/// allocation alive until the module is unloaded.
///
/// [`LoadContext::register_encoder`]: crate::module::LoadContext::register_encoder
pub struct EncoderInfo {
    info: Box<obs_encoder_info>,
}

impl EncoderInfo {
    /// Consumes the wrapper and returns the raw `obs_encoder_info` pointer.
    ///
    /// # Safety
    ///
    /// Transfers ownership of the heap allocation to the caller, which must
    /// later reclaim it via `Box::from_raw`. In normal use this is performed
    /// by [`LoadContext`](crate::module::LoadContext) at module unload; only
    /// call this directly if you are managing registration yourself.
    pub unsafe fn into_raw(self) -> *mut obs_encoder_info {
        Box::into_raw(self.info)
    }
}

impl AsRef<obs_encoder_info> for EncoderInfo {
    fn as_ref(&self) -> &obs_encoder_info {
        self.info.as_ref()
    }
}

/// Builder that wires up the OBS callbacks for a custom encoder.
///
/// Obtain a builder from
/// [`LoadContext::create_encoder_builder`](crate::module::LoadContext::create_encoder_builder)
/// and call the matching `enable_*` method for each optional trait you
/// implemented on `D`. Each `enable_*` method is bounded on the
/// corresponding trait, so the compiler will refuse to enable a callback
/// the encoder cannot service. Finalize with [`build`](Self::build).
pub struct EncoderInfoBuilder<D: Encodable> {
    __data: PhantomData<D>,
    info: obs_encoder_info,
}

impl<D: Encodable> EncoderInfoBuilder<D> {
    pub(crate) fn new() -> Self {
        Self {
            __data: PhantomData,
            info: obs_encoder_info {
                id: D::get_id().as_ptr(),
                type_: D::get_type().as_raw(),
                codec: D::get_codec().as_ptr(),
                create: Some(ffi::create::<D>),
                destroy: Some(ffi::destroy::<D>),
                ..Default::default()
            },
        }
    }

    /// Sets the capability flags advertised to OBS.
    ///
    /// Compose multiple [`EncoderCap`] values with `|`.
    pub fn with_caps(mut self, caps: BitFlags<EncoderCap>) -> Self {
        self.info.caps = caps.bits();
        self
    }

    /// Finalizes the builder into an [`EncoderInfo`] suitable for
    /// [`LoadContext::register_encoder`](crate::module::LoadContext::register_encoder).
    ///
    /// # Panics
    ///
    /// In debug builds, panics if no encode callback has been enabled
    /// (one of [`enable_encode`](Self::enable_encode) or
    /// [`enable_encode_texture`](Self::enable_encode_texture) is required),
    /// or if an audio encoder has not enabled
    /// [`enable_get_frame_size`](Self::enable_get_frame_size).
    pub fn build(self) -> EncoderInfo {
        // Sanity: every encoder must produce packets via *some* encode path.
        debug_assert!(
            self.info.encode.is_some()
                || self.info.encode_texture.is_some()
                || self.info.encode_texture2.is_some(),
            "encoder `{}` has no encode callback — call .enable_encode() or .enable_encode_texture()",
            D::get_id().to_string_lossy(),
        );
        if D::get_type() == EncoderType::Audio {
            debug_assert!(
                self.info.get_frame_size.is_some(),
                "audio encoder `{}` must implement GetFrameSizeEncoder",
                D::get_id().to_string_lossy(),
            );
        }

        EncoderInfo {
            info: Box::new(self.info),
        }
    }
}

macro_rules! impl_encoder_builder {
    ($($f:ident => $t:ident)*) => ($(
        item! {
            impl<D: Encodable + [<$t>]> EncoderInfoBuilder<D> {
                pub fn [<enable_$f>](mut self) -> Self {
                    self.info.[<$f>] = Some(ffi::[<$f>]::<D>);
                    self
                }
            }
        }
    )*)
}

impl_encoder_builder! {
    get_name => GetNameEncoder
    encode => EncodeEncoder
    encode_texture => EncodeTextureEncoder
    update => UpdateEncoder
    get_defaults => GetDefaultsEncoder
    get_properties => GetPropertiesEncoder
    get_extra_data => GetExtraDataEncoder
    get_sei_data => GetSeiDataEncoder
    get_frame_size => GetFrameSizeEncoder
}

/// A composed set of [`EncoderCap`] flags.
///
/// Re-exported so callers can write `EncoderCaps::empty()` and type
/// annotations without depending on `enumflags2` directly. Combining two
/// `EncoderCap` values with `|` already produces an `EncoderCaps`.
pub type EncoderCaps = BitFlags<EncoderCap>;
