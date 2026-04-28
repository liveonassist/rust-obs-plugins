//! Bindings for authoring custom OBS outputs.
//!
//! An output is a sink that consumes raw or encoded audio and video and
//! delivers it elsewhere — to a file, an RTMP server, a virtual camera, and
//! so on. Plugins register outputs at module load time, after which OBS
//! drives them through the standard libobs output lifecycle.
//!
//! # Authoring an output
//!
//! 1. Define a type that holds the per-instance state of your output.
//! 2. Implement [`Outputable`](crate::output::traits::Outputable) for it.
//!    This is mandatory and provides the output identifier as well as the
//!    start/stop hooks.
//! 3. Implement any of the optional traits
//!    ([`GetNameOutput`](crate::output::traits::GetNameOutput),
//!    [`RawVideoOutput`](crate::output::traits::RawVideoOutput),
//!    [`RawAudioOutput`](crate::output::traits::RawAudioOutput),
//!    [`EncodedPacketOutput`](crate::output::traits::EncodedPacketOutput),
//!    [`UpdateOutput`](crate::output::traits::UpdateOutput), …) that match
//!    the OBS callbacks your output needs.
//! 4. Build an [`OutputInfo`](crate::output::OutputInfo) with
//!    [`OutputInfoBuilder`](crate::output::OutputInfoBuilder), opting each
//!    optional trait in with the matching `enable_*` method, and pass it
//!    to [`LoadContext::register_output`].
//!
//! [`OutputInfoBuilder::build`](crate::output::OutputInfoBuilder::build)
//! derives the `OBS_OUTPUT_*` flag set automatically from which callbacks
//! have been enabled.
//!
//! [`LoadContext::register_output`]: crate::module::LoadContext::register_output

use paste::item;

use std::marker::PhantomData;

use obs_sys_rs::{
    OBS_OUTPUT_AUDIO, OBS_OUTPUT_ENCODED, OBS_OUTPUT_MULTI_TRACK, OBS_OUTPUT_VIDEO, obs_output_info,
};

pub mod context;
mod ffi;
pub mod traits;

pub use context::*;
pub use traits::*;

/// A fully-configured output registration, ready to be handed to OBS.
///
/// Produced by [`OutputInfoBuilder::build`] and consumed by
/// [`LoadContext::register_output`], which keeps the underlying allocation
/// alive until the module is unloaded.
///
/// [`LoadContext::register_output`]: crate::module::LoadContext::register_output
pub struct OutputInfo {
    info: Box<obs_output_info>,
}

impl OutputInfo {
    /// Consumes the wrapper and returns the raw `obs_output_info` pointer.
    ///
    /// # Safety
    ///
    /// Transfers ownership of the heap allocation to the caller, which must
    /// later reclaim it via `Box::from_raw`. In normal use this is performed
    /// by [`LoadContext`](crate::module::LoadContext) at module unload.
    pub unsafe fn into_raw(self) -> *mut obs_output_info {
        Box::into_raw(self.info)
    }
}

impl AsRef<obs_output_info> for OutputInfo {
    fn as_ref(&self) -> &obs_output_info {
        self.info.as_ref()
    }
}

/// Builder that wires up the OBS callbacks for a custom output.
///
/// Obtain a builder from
/// [`LoadContext::create_output_builder`](crate::module::LoadContext::create_output_builder)
/// and call the matching `enable_*` method for each optional trait you
/// implemented on `D`. Each `enable_*` method is bounded on the
/// corresponding trait, so the compiler will refuse to enable a callback
/// the output cannot service. Finalize with [`build`](Self::build).
///
/// # Examples
///
/// ```ignore
/// let output = load_context
///     .create_output_builder::<FocusFilter>()
///     .enable_get_name()
///     .enable_raw_video()
///     .build();
/// ```
pub struct OutputInfoBuilder<D: Outputable> {
    __data: PhantomData<D>,
    info: obs_output_info,
}

impl<D: Outputable> OutputInfoBuilder<D> {
    pub(crate) fn new() -> Self {
        Self {
            __data: PhantomData,
            info: obs_output_info {
                id: D::get_id().as_ptr(),
                create: Some(ffi::create::<D>),
                destroy: Some(ffi::destroy::<D>),
                start: Some(ffi::start::<D>),
                stop: Some(ffi::stop::<D>),
                type_data: std::ptr::null_mut(),
                ..Default::default()
            },
        }
    }

    /// Finalizes the builder into an [`OutputInfo`] suitable for
    /// [`LoadContext::register_output`].
    ///
    /// The `OBS_OUTPUT_*` flag set is computed from which callbacks have
    /// been enabled — for example, enabling
    /// [`enable_raw_audio2`](Self::enable_raw_audio2) implies multi-track.
    ///
    /// [`LoadContext::register_output`]: crate::module::LoadContext::register_output
    pub fn build(mut self) -> OutputInfo {
        // see libobs/obs-module.c:obs_register_output_s
        if self.info.encoded_packet.is_some() {
            self.info.flags |= OBS_OUTPUT_ENCODED;
        }

        if self.info.raw_video.is_some() {
            self.info.flags |= OBS_OUTPUT_VIDEO;
        }
        if self.info.raw_audio.is_some() || self.info.raw_audio2.is_some() {
            self.info.flags |= OBS_OUTPUT_AUDIO;
        }
        if self.info.raw_audio2.is_some() {
            self.info.flags |= OBS_OUTPUT_MULTI_TRACK;
        }

        OutputInfo {
            info: Box::new(self.info),
        }
    }
}

macro_rules! impl_output_builder {
    ($($f:ident => $t:ident)*) => ($(
        item! {
            impl<D: Outputable + [<$t>]> OutputInfoBuilder<D> {
                pub fn [<enable_$f>](mut self) -> Self {
                    self.info.[<$f>] = Some(ffi::[<$f>]::<D>);
                    self
                }
            }
        }
    )*)
}

impl_output_builder! {
    get_name => GetNameOutput
    // this two is required
    // start => StartOutput
    // stop => StopOutput
    raw_video => RawVideoOutput
    raw_audio => RawAudioOutput
    raw_audio2 => RawAudio2Output
    encoded_packet => EncodedPacketOutput
    update => UpdateOutput
    get_defaults => GetDefaultsOutput
    // TODO: version?
    // get_defaults2 => GetDefaults2Output
    get_properties => GetPropertiesOutput
    // get_properties2
    // unused1
    get_total_bytes => GetTotalBytesOutput
    get_dropped_frames => GetDroppedFramesOutput
    // type_data
    // free_type_data
    get_congestion => GetCongestionOutput
    get_connect_time_ms => GetConnectTimeMsOutput
}
