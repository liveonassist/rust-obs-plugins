use obs_rs_sys::encoder_packet;

use crate::data::DataObj;

use super::EncoderType;

/// Settings handed to [`Encodable::create`].
///
/// [`Encodable::create`]: super::Encodable::create
pub struct CreatableEncoderContext<'a, D> {
    pub settings: DataObj<'a>,
    pub(crate) _marker: std::marker::PhantomData<fn() -> D>,
}

impl<'a, D> CreatableEncoderContext<'a, D> {
    pub(crate) fn new(settings: DataObj<'a>) -> Self {
        Self {
            settings,
            _marker: std::marker::PhantomData,
        }
    }
}

/// Borrowed view over an `encoder_frame` from libobs. Read-only — encoders
/// don't get to mutate the input.
pub struct EncoderFrame<'a> {
    raw: &'a obs_rs_sys::encoder_frame,
}

impl<'a> EncoderFrame<'a> {
    pub(crate) unsafe fn from_raw(raw: &'a obs_rs_sys::encoder_frame) -> Self {
        Self { raw }
    }

    /// Pointer to plane `idx` (0..8). Use [`plane`](Self::plane) for a slice
    /// view if you know the plane size.
    pub fn plane_ptr(&self, idx: usize) -> *const u8 {
        self.raw.data[idx]
    }

    /// Slice over plane `idx`. The caller specifies `len`: video planes are
    /// `linesize * height_for_plane`; audio is `frames * bytes_per_sample`
    /// (per channel for planar formats).
    ///
    /// # Safety
    /// `len` must not exceed the plane's actual length, and the slice must
    /// not be aliased mutably elsewhere.
    pub unsafe fn plane(&self, idx: usize, len: usize) -> &'a [u8] {
        std::slice::from_raw_parts(self.raw.data[idx], len)
    }

    pub fn linesize(&self, idx: usize) -> u32 {
        self.raw.linesize[idx]
    }

    /// Audio-only: number of audio frames in this buffer.
    pub fn frames(&self) -> u32 {
        self.raw.frames
    }

    pub fn pts(&self) -> i64 {
        self.raw.pts
    }
}

/// Result of an encode call.
#[derive(Debug, Clone, Copy)]
pub enum EncodeStatus {
    /// A packet was produced and written into the [`EncoderPacket`].
    Received,
    /// No packet this call (encoder is buffering).
    NotReady,
}

/// Boxed error type returned from [`EncodeEncoder::encode`] /
/// [`EncodeTextureEncoder::encode_texture`]. Same shape as `CreateError` —
/// any `Display + Send + Sync` error converts via `Into`.
///
/// [`EncodeEncoder::encode`]: super::traits::EncodeEncoder::encode
/// [`EncodeTextureEncoder::encode_texture`]: super::traits::EncodeTextureEncoder::encode_texture
pub type EncodeError = Box<dyn std::error::Error + Send + Sync>;

/// A typed view onto the `encoder_packet` libobs handed us, plus a
/// per-encoder owned buffer the user writes payload bytes into.
///
/// The buffer is owned by the encoder wrapper (one `Vec<u8>` per encoder
/// instance, reused across calls). Each call should:
/// 1. [`reset`](Self::reset) the buffer.
/// 2. Set timestamps via [`set_pts`](Self::set_pts) / [`set_dts`](Self::set_dts) /
///    [`set_keyframe`](Self::set_keyframe) and any other fields.
/// 3. Append payload bytes via [`write`](Self::write) (or [`writer`](Self::writer)
///    for `Vec`-style construction).
/// 4. Return [`EncodeStatus::Received`].
///
/// On return the FFI shim points `encoder_packet.data` at the buffer; OBS
/// reads it before the next encode call, then drops its reference.
pub struct EncoderPacket<'a> {
    pub(crate) raw: &'a mut encoder_packet,
    pub(crate) buffer: &'a mut Vec<u8>,
}

impl EncoderPacket<'_> {
    /// Truncate the per-encoder payload buffer. Cheap; capacity is retained
    /// across calls.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Append bytes to the payload.
    pub fn write(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Direct mutable access to the payload buffer. Useful when the codec
    /// hands you a buffer to extend in-place (e.g. NAL splicing for SEI).
    pub fn writer(&mut self) -> &mut Vec<u8> {
        self.buffer
    }

    pub fn set_pts(&mut self, pts: i64) {
        self.raw.pts = pts;
    }

    pub fn set_dts(&mut self, dts: i64) {
        self.raw.dts = dts;
    }

    pub fn set_keyframe(&mut self, keyframe: bool) {
        self.raw.keyframe = keyframe;
    }

    pub fn set_priority(&mut self, priority: i32) {
        self.raw.priority = priority;
    }

    pub fn set_drop_priority(&mut self, drop_priority: i32) {
        self.raw.drop_priority = drop_priority;
    }

    pub fn set_track_idx(&mut self, idx: usize) {
        self.raw.track_idx = idx;
    }

    pub fn set_timebase(&mut self, num: i32, den: i32) {
        self.raw.timebase_num = num;
        self.raw.timebase_den = den;
    }

    pub fn set_packet_type(&mut self, ty: EncoderType) {
        self.raw.type_ = ty.as_raw();
    }
}

/// Read-only view over an `encoder_packet` an output is receiving (e.g.
/// from a downstream encoder). The underlying buffer is owned by the
/// encoder; the view only lives for the duration of the callback.
pub struct EncodedPacketView<'a> {
    raw: &'a encoder_packet,
}

impl<'a> EncodedPacketView<'a> {
    pub(crate) unsafe fn from_raw(raw: &'a encoder_packet) -> Self {
        Self { raw }
    }

    /// Encoded payload bytes.
    pub fn data(&self) -> &'a [u8] {
        // SAFETY: raw.data + raw.size is the encoder's owned buffer; valid for
        // the lifetime of this view.
        unsafe { std::slice::from_raw_parts(self.raw.data, self.raw.size) }
    }

    pub fn pts(&self) -> i64 {
        self.raw.pts
    }

    pub fn dts(&self) -> i64 {
        self.raw.dts
    }

    pub fn keyframe(&self) -> bool {
        self.raw.keyframe
    }

    pub fn priority(&self) -> i32 {
        self.raw.priority
    }

    pub fn drop_priority(&self) -> i32 {
        self.raw.drop_priority
    }

    pub fn track_idx(&self) -> usize {
        self.raw.track_idx
    }

    pub fn timebase(&self) -> (i32, i32) {
        (self.raw.timebase_num, self.raw.timebase_den)
    }

    pub fn packet_type(&self) -> Option<EncoderType> {
        match self.raw.type_ {
            obs_rs_sys::obs_encoder_type_OBS_ENCODER_VIDEO => Some(EncoderType::Video),
            obs_rs_sys::obs_encoder_type_OBS_ENCODER_AUDIO => Some(EncoderType::Audio),
            _ => None,
        }
    }
}
