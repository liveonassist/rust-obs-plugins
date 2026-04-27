//! Push API for asynchronous input sources.
//!
//! Async sources hand decoded frames to OBS from their own thread (e.g. a
//! network receiver, a capture device worker). They register without a
//! `video_render` callback and instead call [`AsyncVideoSource::output_video`]
//! / [`AsyncAudioSource::output_audio`].
//!
//! Get a handle from [`SourceRef::as_async_video`] /
//! [`SourceRef::as_async_audio`] inside [`Sourceable::create`]; both check
//! the source's `output_flags` at runtime and return `None` if the source
//! wasn't registered with the matching `enable_async_*` builder method.
//! Both handles are `Send + Sync` and refcount the underlying source, so
//! they can be cloned and moved into worker threads.
//!
//! [`Sourceable::create`]: super::Sourceable::create

use obs_rs_sys::{
    audio_format, audio_format_AUDIO_FORMAT_16BIT, audio_format_AUDIO_FORMAT_16BIT_PLANAR,
    audio_format_AUDIO_FORMAT_32BIT, audio_format_AUDIO_FORMAT_32BIT_PLANAR,
    audio_format_AUDIO_FORMAT_FLOAT, audio_format_AUDIO_FORMAT_FLOAT_PLANAR,
    audio_format_AUDIO_FORMAT_U8BIT, audio_format_AUDIO_FORMAT_U8BIT_PLANAR, obs_source_audio,
    obs_source_frame, obs_source_get_output_flags, obs_source_get_ref, obs_source_output_audio,
    obs_source_output_video, obs_source_release, obs_source_t, speaker_layout,
    speaker_layout_SPEAKERS_2POINT1, speaker_layout_SPEAKERS_4POINT0,
    speaker_layout_SPEAKERS_4POINT1, speaker_layout_SPEAKERS_5POINT1, speaker_layout_SPEAKERS_7POINT1,
    speaker_layout_SPEAKERS_MONO, speaker_layout_SPEAKERS_STEREO, speaker_layout_SPEAKERS_UNKNOWN,
    OBS_SOURCE_ASYNC_VIDEO, OBS_SOURCE_AUDIO,
};

use crate::media::video::VideoFormat;
use crate::native_enum;
use crate::wrapper::PtrWrapper;

use super::SourceRef;

native_enum!(AudioFormat, audio_format {
    U8Bit => AUDIO_FORMAT_U8BIT,
    Bit16 => AUDIO_FORMAT_16BIT,
    Bit32 => AUDIO_FORMAT_32BIT,
    Float => AUDIO_FORMAT_FLOAT,
    U8BitPlanar => AUDIO_FORMAT_U8BIT_PLANAR,
    Bit16Planar => AUDIO_FORMAT_16BIT_PLANAR,
    Bit32Planar => AUDIO_FORMAT_32BIT_PLANAR,
    FloatPlanar => AUDIO_FORMAT_FLOAT_PLANAR,
});

native_enum!(Speakers, speaker_layout {
    Unknown => SPEAKERS_UNKNOWN,
    Mono => SPEAKERS_MONO,
    Stereo => SPEAKERS_STEREO,
    TwoPoint1 => SPEAKERS_2POINT1,
    FourPoint0 => SPEAKERS_4POINT0,
    FourPoint1 => SPEAKERS_4POINT1,
    FivePoint1 => SPEAKERS_5POINT1,
    SevenPoint1 => SPEAKERS_7POINT1,
});

/// One plane of a video frame: a buffer plus its row stride in bytes.
///
/// For multi-plane formats (e.g. NV12, I420), each plane describes a separate
/// image-component buffer (Y, UV, or Y/U/V respectively).
#[derive(Clone, Copy, Default)]
pub struct Plane<'a> {
    pub data: &'a [u8],
    pub linesize: u32,
}

/// A video frame ready to be pushed into OBS.
///
/// Construct with [`SourceVideoFrame::new`] and add planes with
/// [`SourceVideoFrame::with_plane`]. OBS copies the buffers internally, so the
/// borrow only needs to outlive the [`AsyncVideoSource::output_video`] call.
pub struct SourceVideoFrame<'a> {
    /// Pixel format. Must match the layout of `planes`.
    pub format: VideoFormat,
    pub width: u32,
    pub height: u32,
    /// Presentation timestamp in OBS's nanosecond domain (`os_gettime_ns`).
    pub timestamp_ns: u64,
    /// Up to 8 planes. Index 0 carries the primary plane for single-plane
    /// formats; multi-plane layouts use 0..N as the codec dictates (NV12: 0=Y,
    /// 1=UV; I420: 0=Y, 1=U, 2=V).
    pub planes: [Plane<'a>; 8],
    /// Flip vertically when rendering.
    pub flip: bool,
    /// `true` for full-range YUV, `false` for studio/limited range. Ignored
    /// for RGB formats.
    pub full_range: bool,
}

impl<'a> SourceVideoFrame<'a> {
    pub fn new(format: VideoFormat, width: u32, height: u32, timestamp_ns: u64) -> Self {
        Self {
            format,
            width,
            height,
            timestamp_ns,
            planes: Default::default(),
            flip: false,
            full_range: false,
        }
    }

    /// Set plane `idx`. Panics if `idx >= 8`.
    pub fn with_plane(mut self, idx: usize, data: &'a [u8], linesize: u32) -> Self {
        self.planes[idx] = Plane { data, linesize };
        self
    }

    pub fn with_flip(mut self, flip: bool) -> Self {
        self.flip = flip;
        self
    }

    pub fn with_full_range(mut self, full_range: bool) -> Self {
        self.full_range = full_range;
        self
    }
}

/// An audio buffer ready to be pushed into OBS.
///
/// For interleaved formats fill `planes[0]`. For planar formats fill
/// `planes[0..channel_count]`. Each plane must contain exactly
/// `frames * bytes_per_sample` bytes (per channel for planar; total for
/// interleaved). OBS copies internally.
pub struct SourceAudio<'a> {
    pub format: AudioFormat,
    pub speakers: Speakers,
    pub samples_per_sec: u32,
    pub frames: u32,
    /// Presentation timestamp in OBS's nanosecond domain (`os_gettime_ns`).
    pub timestamp_ns: u64,
    pub planes: [&'a [u8]; 8],
}

impl<'a> SourceAudio<'a> {
    pub fn new(
        format: AudioFormat,
        speakers: Speakers,
        samples_per_sec: u32,
        frames: u32,
        timestamp_ns: u64,
    ) -> Self {
        Self {
            format,
            speakers,
            samples_per_sec,
            frames,
            timestamp_ns,
            planes: [&[]; 8],
        }
    }

    /// Set plane `idx`. Panics if `idx >= 8`.
    pub fn with_plane(mut self, idx: usize, data: &'a [u8]) -> Self {
        self.planes[idx] = data;
        self
    }
}

/// A `Send + Sync` handle to a source registered with `OBS_SOURCE_ASYNC_VIDEO`.
///
/// Obtain via [`SourceRef::as_async_video`]. The only operation is
/// [`AsyncVideoSource::output_video`], which OBS documents as thread-safe.
/// Refcounted; `Clone` is cheap and safe.
pub struct AsyncVideoSource {
    inner: *mut obs_source_t,
}

// SAFETY: `obs_source_output_video` is thread-safe (it takes the source's
// `async_mutex` internally; see libobs/obs-source.c). `obs_source_get_ref`
// and `obs_source_release` use atomic refcounts.
unsafe impl Send for AsyncVideoSource {}
unsafe impl Sync for AsyncVideoSource {}

impl_ptr_wrapper!(
    @ptr: inner,
    AsyncVideoSource,
    obs_source_t,
    obs_source_get_ref,
    obs_source_release
);

impl AsyncVideoSource {
    /// Push a frame to the source's async video queue. OBS copies the buffer
    /// data; planes only need to live for the duration of this call.
    pub fn output_video(&self, frame: &SourceVideoFrame<'_>) {
        let mut raw: obs_source_frame = unsafe { std::mem::zeroed() };
        for i in 0..8 {
            raw.data[i] = frame.planes[i].data.as_ptr() as *mut u8;
            raw.linesize[i] = frame.planes[i].linesize;
        }
        raw.width = frame.width;
        raw.height = frame.height;
        raw.timestamp = frame.timestamp_ns;
        raw.format = frame.format.as_raw();
        raw.flip = frame.flip;
        raw.full_range = frame.full_range;
        unsafe { obs_source_output_video(self.inner, &raw) }
    }
}

/// A `Send + Sync` handle to a source registered with `OBS_SOURCE_AUDIO`.
///
/// Obtain via [`SourceRef::as_async_audio`]. The only operation is
/// [`AsyncAudioSource::output_audio`]; thread-safety guarantees match
/// [`AsyncVideoSource`].
pub struct AsyncAudioSource {
    inner: *mut obs_source_t,
}

// SAFETY: see AsyncVideoSource.
unsafe impl Send for AsyncAudioSource {}
unsafe impl Sync for AsyncAudioSource {}

impl_ptr_wrapper!(
    @ptr: inner,
    AsyncAudioSource,
    obs_source_t,
    obs_source_get_ref,
    obs_source_release
);

impl AsyncAudioSource {
    /// Push an audio buffer to the source's audio queue. OBS copies internally.
    pub fn output_audio(&self, audio: &SourceAudio<'_>) {
        let mut raw: obs_source_audio = unsafe { std::mem::zeroed() };
        for i in 0..8 {
            raw.data[i] = audio.planes[i].as_ptr();
        }
        raw.frames = audio.frames;
        raw.speakers = audio.speakers.as_raw();
        raw.format = audio.format.as_raw();
        raw.samples_per_sec = audio.samples_per_sec;
        raw.timestamp = audio.timestamp_ns;
        unsafe { obs_source_output_audio(self.inner, &raw) }
    }
}

impl SourceRef {
    /// Returns a thread-safe push handle if this source was registered with
    /// `enable_async_video` (i.e. its `output_flags` set `OBS_SOURCE_ASYNC_VIDEO`).
    ///
    /// Returns `None` for non-async or render-callback sources — there is no
    /// `async_frames` consumer on the render side, so pushed frames would be
    /// silently dropped.
    pub fn as_async_video(&self) -> Option<AsyncVideoSource> {
        let flags = unsafe { obs_source_get_output_flags(self.as_ptr()) };
        if (flags & OBS_SOURCE_ASYNC_VIDEO) != OBS_SOURCE_ASYNC_VIDEO {
            return None;
        }
        // SAFETY: SourceRef holds a valid (refcounted) source pointer.
        unsafe { AsyncVideoSource::from_raw(self.as_ptr_mut()) }
    }

    /// Returns a thread-safe push handle if this source was registered with
    /// `enable_async_audio` (i.e. its `output_flags` set `OBS_SOURCE_AUDIO`).
    pub fn as_async_audio(&self) -> Option<AsyncAudioSource> {
        let flags = unsafe { obs_source_get_output_flags(self.as_ptr()) };
        if (flags & OBS_SOURCE_AUDIO) != OBS_SOURCE_AUDIO {
            return None;
        }
        unsafe { AsyncAudioSource::from_raw(self.as_ptr_mut()) }
    }
}
