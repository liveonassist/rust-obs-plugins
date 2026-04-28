use obs_sys_rs::encoder_packet;

use crate::data::DataObj;

use super::EncoderType;

/// Construction-time context handed to [`Encodable::create`].
///
/// Carries the initial [`DataObj`] settings supplied by OBS when an encoder
/// instance is being created. The type parameter `D` is the implementing
/// encoder type, used to keep callbacks routed to the correct `Encodable`
/// implementation.
///
/// [`Encodable::create`]: super::Encodable::create
pub struct CreatableEncoderContext<'a, D> {
    /// Initial encoder settings.
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

/// A borrowed, read-only view of an input frame supplied by OBS.
///
/// `EncoderFrame` exposes the planes, line strides, frame count, and
/// presentation timestamp of an input buffer that libobs is asking the
/// encoder to consume. The underlying memory is owned by OBS and is only
/// valid for the duration of the [`EncodeEncoder::encode`] call.
///
/// [`EncodeEncoder::encode`]: super::traits::EncodeEncoder::encode
pub struct EncoderFrame<'a> {
    raw: &'a obs_sys_rs::encoder_frame,
}

impl<'a> EncoderFrame<'a> {
    pub(crate) unsafe fn from_raw(raw: &'a obs_sys_rs::encoder_frame) -> Self {
        Self { raw }
    }

    /// Returns a raw pointer to plane `idx`. Up to 8 planes may be present
    /// (`0..8`); empty planes return a null pointer.
    ///
    /// Prefer [`plane`](Self::plane) when you can compute the plane size
    /// up front.
    pub fn plane_ptr(&self, idx: usize) -> *const u8 {
        self.raw.data[idx]
    }

    /// Returns plane `idx` as a byte slice of length `len`.
    ///
    /// For video planes, `len` is typically `linesize * height_for_plane`;
    /// for audio, `frames * bytes_per_sample` (per channel for planar
    /// formats).
    ///
    /// # Safety
    ///
    /// `len` must not exceed the plane's actual length, and no other live
    /// reference may alias this region for the lifetime of the returned
    /// slice.
    pub unsafe fn plane(&self, idx: usize, len: usize) -> &'a [u8] {
        std::slice::from_raw_parts(self.raw.data[idx], len)
    }

    /// Returns the row stride in bytes for plane `idx`.
    pub fn linesize(&self, idx: usize) -> u32 {
        self.raw.linesize[idx]
    }

    /// Returns the number of audio frames in this buffer (audio only).
    pub fn frames(&self) -> u32 {
        self.raw.frames
    }

    /// Returns the presentation timestamp of this frame, in the encoder's
    /// timebase.
    pub fn pts(&self) -> i64 {
        self.raw.pts
    }
}

/// The outcome of a single encode call.
#[derive(Debug, Clone, Copy)]
pub enum EncodeStatus {
    /// A packet was produced and written into the [`EncoderPacket`].
    Received,
    /// No packet was produced this call. The encoder is still buffering
    /// input and is not yet ready to emit output.
    NotReady,
}

/// The error type returned from encode callbacks.
///
/// Any `Display + Send + Sync` error type converts into `EncodeError` via
/// the standard `Into` impl, mirroring [`CreateError`](crate::source::traits::CreateError).
///
/// [`EncodeEncoder::encode`]: super::traits::EncodeEncoder::encode
/// [`EncodeTextureEncoder::encode_texture`]: super::traits::EncodeTextureEncoder::encode_texture
pub type EncodeError = Box<dyn std::error::Error + Send + Sync>;

/// A mutable view of the packet OBS is filling, paired with the
/// encoder-owned payload buffer.
///
/// The payload buffer is reused across calls (one `Vec<u8>` per encoder
/// instance). A typical encode callback:
///
/// 1. Calls [`reset`](Self::reset) to clear the previous payload.
/// 2. Sets timestamps and codec metadata with [`set_pts`](Self::set_pts),
///    [`set_dts`](Self::set_dts), [`set_keyframe`](Self::set_keyframe), and
///    related setters.
/// 3. Appends payload bytes via [`write`](Self::write), or constructs them
///    in place via [`writer`](Self::writer).
/// 4. Returns [`EncodeStatus::Received`].
///
/// When the callback returns, the FFI shim points `encoder_packet.data` at
/// the buffer's bytes. OBS reads them before the next encode call, after
/// which the buffer can be reused.
pub struct EncoderPacket<'a> {
    pub(crate) raw: &'a mut encoder_packet,
    pub(crate) buffer: &'a mut Vec<u8>,
}

impl EncoderPacket<'_> {
    /// Clears the payload buffer without releasing its capacity.
    ///
    /// Call this at the top of every encode callback before writing the
    /// new payload.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Appends bytes to the payload.
    pub fn write(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// Returns mutable access to the payload buffer.
    ///
    /// Useful when the underlying codec hands back a buffer that should be
    /// extended in place — for example, when splicing NAL units for SEI
    /// insertion.
    pub fn writer(&mut self) -> &mut Vec<u8> {
        self.buffer
    }

    /// Sets the presentation timestamp, in the encoder's timebase.
    pub fn set_pts(&mut self, pts: i64) {
        self.raw.pts = pts;
    }

    /// Sets the decode timestamp, in the encoder's timebase.
    pub fn set_dts(&mut self, dts: i64) {
        self.raw.dts = dts;
    }

    /// Marks this packet as a keyframe.
    pub fn set_keyframe(&mut self, keyframe: bool) {
        self.raw.keyframe = keyframe;
    }

    /// Sets the muxing priority of this packet.
    pub fn set_priority(&mut self, priority: i32) {
        self.raw.priority = priority;
    }

    /// Sets the priority used to decide which packets to drop under
    /// congestion.
    pub fn set_drop_priority(&mut self, drop_priority: i32) {
        self.raw.drop_priority = drop_priority;
    }

    /// Sets the track index this packet belongs to (multi-track outputs).
    pub fn set_track_idx(&mut self, idx: usize) {
        self.raw.track_idx = idx;
    }

    /// Sets the rational timebase for the timestamps on this packet.
    pub fn set_timebase(&mut self, num: i32, den: i32) {
        self.raw.timebase_num = num;
        self.raw.timebase_den = den;
    }

    /// Marks this packet as audio or video.
    pub fn set_packet_type(&mut self, ty: EncoderType) {
        self.raw.type_ = ty.as_raw();
    }
}

/// A read-only view of a previously-encoded packet, as delivered to an
/// output.
///
/// Outputs that consume encoded packets receive an `EncodedPacketView`
/// referring to a buffer owned by the upstream encoder. The view is only
/// valid for the duration of the receiving callback; copy any bytes you
/// need to retain.
pub struct EncodedPacketView<'a> {
    raw: &'a encoder_packet,
}

impl<'a> EncodedPacketView<'a> {
    pub(crate) unsafe fn from_raw(raw: &'a encoder_packet) -> Self {
        Self { raw }
    }

    /// Returns the encoded payload bytes.
    pub fn data(&self) -> &'a [u8] {
        // SAFETY: raw.data + raw.size is the encoder's owned buffer; valid for
        // the lifetime of this view.
        unsafe { std::slice::from_raw_parts(self.raw.data, self.raw.size) }
    }

    /// Returns the presentation timestamp.
    pub fn pts(&self) -> i64 {
        self.raw.pts
    }

    /// Returns the decode timestamp.
    pub fn dts(&self) -> i64 {
        self.raw.dts
    }

    /// Returns whether this packet is marked as a keyframe.
    pub fn keyframe(&self) -> bool {
        self.raw.keyframe
    }

    /// Returns the muxing priority.
    pub fn priority(&self) -> i32 {
        self.raw.priority
    }

    /// Returns the priority used by congestion-based packet drop.
    pub fn drop_priority(&self) -> i32 {
        self.raw.drop_priority
    }

    /// Returns the track index this packet belongs to.
    pub fn track_idx(&self) -> usize {
        self.raw.track_idx
    }

    /// Returns the timebase as a `(numerator, denominator)` pair.
    pub fn timebase(&self) -> (i32, i32) {
        (self.raw.timebase_num, self.raw.timebase_den)
    }

    /// Returns whether this packet is audio or video, or `None` if the
    /// underlying value does not correspond to a known encoder type.
    pub fn packet_type(&self) -> Option<EncoderType> {
        match self.raw.type_ {
            obs_sys_rs::obs_encoder_type_OBS_ENCODER_VIDEO => Some(EncoderType::Video),
            obs_sys_rs::obs_encoder_type_OBS_ENCODER_AUDIO => Some(EncoderType::Audio),
            _ => None,
        }
    }
}
