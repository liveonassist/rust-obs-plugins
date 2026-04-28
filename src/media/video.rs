use obs_sys_rs::{
    obs_source_frame, video_data, video_format, video_format_VIDEO_FORMAT_AYUV,
    video_format_VIDEO_FORMAT_BGR3, video_format_VIDEO_FORMAT_BGRA, video_format_VIDEO_FORMAT_BGRX,
    video_format_VIDEO_FORMAT_I010, video_format_VIDEO_FORMAT_I40A, video_format_VIDEO_FORMAT_I42A,
    video_format_VIDEO_FORMAT_I210, video_format_VIDEO_FORMAT_I412, video_format_VIDEO_FORMAT_I420,
    video_format_VIDEO_FORMAT_I422, video_format_VIDEO_FORMAT_I444, video_format_VIDEO_FORMAT_NONE,
    video_format_VIDEO_FORMAT_NV12, video_format_VIDEO_FORMAT_P010, video_format_VIDEO_FORMAT_RGBA,
    video_format_VIDEO_FORMAT_UYVY, video_format_VIDEO_FORMAT_Y800, video_format_VIDEO_FORMAT_YA2L,
    video_format_VIDEO_FORMAT_YUVA, video_format_VIDEO_FORMAT_YUY2, video_format_VIDEO_FORMAT_YVYU,
    video_output_get_format, video_output_get_frame_rate, video_output_get_height,
    video_output_get_width, video_t,
};

use crate::native_enum;

native_enum!(VideoFormat, video_format {
    None => VIDEO_FORMAT_NONE,
    /// planar 4:2:0 formats, three-plane
    I420 => VIDEO_FORMAT_I420,
    /// planar 4:2:0 formats, two-plane, luma and packed chroma
    NV12 => VIDEO_FORMAT_NV12,

    /// packed 4:2:2 formats
    YVYU => VIDEO_FORMAT_YVYU,
    /// packed 4:2:2 formats, YUYV
    YUY2 => VIDEO_FORMAT_YUY2,
    /// packed 4:2:2 formats
    UYVY => VIDEO_FORMAT_UYVY,

    /// packed uncompressed formats
    RGBA => VIDEO_FORMAT_RGBA,
    /// packed uncompressed formats
    BGRA => VIDEO_FORMAT_BGRA,
    /// packed uncompressed formats
    BGRX => VIDEO_FORMAT_BGRX,
    /// packed uncompressed formats, grayscale
    Y800 => VIDEO_FORMAT_Y800,

    /// planar 4:4:4
    I444 => VIDEO_FORMAT_I444,
    /// more packed uncompressed formats
    BGR3 => VIDEO_FORMAT_BGR3,
    /// planar 4:2:2
    I422 => VIDEO_FORMAT_I422,
    /// planar 4:2:0 with alpha
    I40A => VIDEO_FORMAT_I40A,
    /// planar 4:2:2 with alpha
    I42A => VIDEO_FORMAT_I42A,
    /// planar 4:4:4 with alpha
    YUVA => VIDEO_FORMAT_YUVA,
    /// packed 4:4:4 with alpha
    AYUV => VIDEO_FORMAT_AYUV,

    /// planar 4:2:0 format, 10 bpp, three-plane
    I010 => VIDEO_FORMAT_I010,
    /// planar 4:2:0 format, 10 bpp, two-plane, luma and packed chroma
    P010 => VIDEO_FORMAT_P010,
    /// planar 4:2:2 10 bits, Little Endian
    I210 => VIDEO_FORMAT_I210,
    /// planar 4:4:4 12 bits, Little Endian
    I412 => VIDEO_FORMAT_I412,
    /// planar 4:4:4 12 bits with alpha, Little Endian
    YA2L => VIDEO_FORMAT_YA2L,
});

/// A view of a video frame flowing through a source's filter callback.
///
/// `VideoDataSourceContext` wraps the `obs_source_frame` libobs hands to
/// CPU-side video filters such as
/// [`FilterVideoSource::filter_video`](crate::source::traits::FilterVideoSource::filter_video).
///
/// [`AsyncVideoSource::output_video`]: crate::source::push::AsyncVideoSource::output_video
pub struct VideoDataSourceContext {
    pointer: *mut obs_source_frame,
}

impl VideoDataSourceContext {
    /// Wraps a raw `obs_source_frame*`.
    pub fn from_raw(pointer: *mut obs_source_frame) -> Self {
        Self { pointer }
    }

    /// Returns the frame's pixel format, or `None` if libobs reports an
    /// unknown value.
    pub fn format(&self) -> Option<VideoFormat> {
        let raw = unsafe { (*self.pointer).format };

        VideoFormat::from_raw(raw).ok()
    }

    /// Returns the frame width in pixels.
    pub fn width(&self) -> u32 {
        unsafe { (*self.pointer).width }
    }

    /// Returns the frame height in pixels.
    pub fn height(&self) -> u32 {
        unsafe { (*self.pointer).height }
    }

    /// Returns a raw pointer to plane `idx`.
    pub fn data_buffer(&self, idx: usize) -> *mut u8 {
        unsafe { (*self.pointer).data[idx] }
    }

    /// Returns the row stride in bytes for plane `idx`.
    pub fn linesize(&self, idx: usize) -> u32 {
        unsafe { (*self.pointer).linesize[idx] }
    }

    /// Returns the frame's presentation timestamp, in nanoseconds.
    pub fn timestamp(&self) -> u64 {
        unsafe { (*self.pointer).timestamp }
    }
}

/// A view of the video buffer libobs delivers to an output's
/// [`RawVideoOutput::raw_video`] callback.
///
/// [`RawVideoOutput::raw_video`]: crate::output::traits::RawVideoOutput::raw_video
pub struct VideoDataOutputContext {
    pointer: *mut video_data,
}

impl VideoDataOutputContext {
    /// Wraps a raw `video_data*`.
    pub fn from_raw(pointer: *mut video_data) -> Self {
        Self { pointer }
    }

    /// Returns a raw pointer to plane `idx`.
    pub fn data_buffer(&self, idx: usize) -> *mut u8 {
        unsafe { (*self.pointer).data[idx] }
    }

    /// Returns the row stride in bytes for plane `idx`.
    pub fn linesize(&self, idx: usize) -> u32 {
        unsafe { (*self.pointer).linesize[idx] }
    }

    /// Returns the buffer's presentation timestamp, in nanoseconds.
    pub fn timestamp(&self) -> u64 {
        unsafe { (*self.pointer).timestamp }
    }
}

/// Owned snapshot of a video output's configuration.
#[allow(unused)]
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Output frame rate.
    pub frame_rate: f64,
    /// Pixel format, or `None` if libobs reports an unknown value.
    pub format: Option<VideoFormat>,
}

/// Per-format description of a video frame's plane layout.
///
/// Returned by [`VideoInfo::frame_size`]; used by callers that need to
/// allocate or size buffers for a frame in a specific [`VideoFormat`].
pub enum FrameSize {
    /// The format is unknown or has zero planes.
    Unknown,
    /// `count` planes, each of `size` bytes.
    Planes { size: usize, count: usize },
    /// A single plane of the given size.
    OnePlane(usize),
    /// Two planes of the given sizes.
    TwoPlane(usize, usize),
    /// Three planes of the given sizes.
    ThreePlane(usize, usize, usize),
    /// Four planes of the given sizes.
    FourPlane(usize, usize, usize, usize),
}

impl VideoInfo {
    /// Returns the per-plane buffer size for this video configuration.
    ///
    /// The layout follows libobs's `video-frame.c` reference; see the
    /// [upstream source][src] for the authoritative formulae.
    ///
    /// [src]: https://github.com/obsproject/obs-studio/blob/a1e8075fba09f3b56ed43ead64cc3e340dd7a059/libobs/media-io/video-frame.c#L23
    pub fn frame_size(&self) -> FrameSize {
        use VideoFormat::*;
        let width = self.width as usize;
        let height = self.height as usize;
        let half_width = width.div_ceil(2);
        let half_height = height.div_ceil(2);
        let full_size = width * height;
        let half_size = half_width * height;
        let quarter_size = half_width * half_height;
        let Some(format) = self.format else {
            return FrameSize::Unknown;
        };
        match format {
            VideoFormat::None => FrameSize::Planes { size: 0, count: 0 },
            I420 => FrameSize::ThreePlane(full_size, quarter_size, quarter_size),
            NV12 => FrameSize::TwoPlane(full_size, half_size * 2),
            Y800 => FrameSize::OnePlane(full_size),
            YVYU | UYVY | YUY2 => FrameSize::OnePlane(half_size * 4),
            BGRX | BGRA | RGBA | AYUV => FrameSize::OnePlane(full_size * 4),
            I444 => FrameSize::Planes {
                count: 3,
                size: full_size,
            },
            I412 => FrameSize::Planes {
                count: 3,
                size: full_size * 2,
            },
            BGR3 => FrameSize::OnePlane(full_size * 3),
            I422 => FrameSize::ThreePlane(full_size, half_size, half_size),
            I210 => FrameSize::ThreePlane(full_size * 2, half_size * 2, half_size * 2),
            I40A => FrameSize::FourPlane(full_size, quarter_size, quarter_size, full_size),
            I42A => FrameSize::FourPlane(full_size, half_size, half_size, full_size),
            YUVA => FrameSize::Planes {
                count: 4,
                size: full_size,
            },
            YA2L => FrameSize::Planes {
                count: 4,
                size: full_size * 2,
            },
            I010 => FrameSize::ThreePlane(full_size * 2, quarter_size * 2, quarter_size * 2),
            P010 => FrameSize::TwoPlane(full_size * 2, quarter_size * 4),
        }
    }
}

/// A handle to an OBS video output (`video_t`).
///
/// `VideoRef` exposes read-only inspection of the video output's
/// dimensions, frame rate, and pixel format. It is not reference-counted;
/// libobs owns the underlying object.
#[allow(unused)]
pub struct VideoRef {
    /// Pointer to the underlying `video_t`.
    pub pointer: *mut video_t,
}

#[allow(unused)]
impl VideoRef {
    /// Wraps a raw `video_t*`.
    pub fn from_raw(pointer: *mut video_t) -> Self {
        Self { pointer }
    }

    /// Returns a snapshot of the video output's configuration.
    pub fn info(&self) -> VideoInfo {
        VideoInfo {
            width: self.width(),
            height: self.height(),
            frame_rate: self.frame_rate(),
            format: self.format(),
        }
    }

    /// Returns the output width, in pixels.
    pub fn width(&self) -> u32 {
        unsafe { video_output_get_width(self.pointer) }
    }

    /// Returns the output height, in pixels.
    pub fn height(&self) -> u32 {
        unsafe { video_output_get_height(self.pointer) }
    }

    /// Returns the output frame rate.
    pub fn frame_rate(&self) -> f64 {
        unsafe { video_output_get_frame_rate(self.pointer) }
    }

    /// Returns the pixel format, or `None` if libobs reports an unknown
    /// value.
    pub fn format(&self) -> Option<VideoFormat> {
        let raw = unsafe { video_output_get_format(self.pointer) };

        VideoFormat::from_raw(raw).ok()
    }
}
