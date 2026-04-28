use obs_sys_rs::{
    audio_data, audio_output_get_channels, audio_output_get_sample_rate, audio_t, obs_audio_data,
};

/// A mutable view of an audio buffer flowing through a filter callback.
///
/// `AudioDataContext` wraps the `obs_audio_data` libobs hands to filter
/// callbacks such as
/// [`FilterAudioSource::filter_audio`](crate::source::traits::FilterAudioSource::filter_audio).
/// The buffer is laid out as a fixed number of planar channels with a
/// shared frame count.
pub struct AudioDataContext {
    pointer: *mut obs_audio_data,
}

impl AudioDataContext {
    /// Wraps a raw `obs_audio_data*`.
    pub fn from_raw(pointer: *mut obs_audio_data) -> Self {
        Self { pointer }
    }

    /// Returns the number of audio frames in the buffer.
    pub fn frames(&self) -> usize {
        unsafe {
            self.pointer
                .as_ref()
                .expect("Audio pointer was null!")
                .frames as usize
        }
    }

    /// Returns the number of channels in the buffer.
    pub fn channels(&self) -> usize {
        unsafe {
            self.pointer
                .as_ref()
                .expect("Audio pointer was null!")
                .data
                .len()
        }
    }

    /// Returns a mutable `f32` slice over the named channel.
    ///
    /// Returns `None` if `channel` is out of range. Mutations to the
    /// slice are visible to downstream filters and outputs.
    pub fn get_channel_as_mut_slice(&mut self, channel: usize) -> Option<&'_ mut [f32]> {
        unsafe {
            let data = self.pointer.as_ref()?.data;

            if channel >= data.len() {
                return None;
            }

            let frames = self.pointer.as_ref()?.frames;

            Some(core::slice::from_raw_parts_mut(
                data[channel] as *mut f32,
                frames as usize,
            ))
        }
    }
}

/// A view of the audio buffer libobs delivers to an output's
/// [`RawAudioOutput::raw_audio`] callback.
///
/// Format and sample rate are determined by the [`AudioRef`] this output
/// is bound to.
///
/// [`RawAudioOutput::raw_audio`]: crate::output::traits::RawAudioOutput::raw_audio
pub struct AudioDataOutputContext {
    pointer: *mut audio_data,
}

impl AudioDataOutputContext {
    /// Wraps a raw `audio_data*`.
    pub fn from_raw(pointer: *mut audio_data) -> Self {
        Self { pointer }
    }

    /// Returns a raw pointer to plane `idx`.
    pub fn data_buffer(&self, idx: usize) -> *mut u8 {
        unsafe { (*self.pointer).data[idx] }
    }

    /// Returns the number of audio frames in the buffer.
    pub fn frames(&self) -> u32 {
        unsafe { (*self.pointer).frames }
    }

    /// Returns the buffer's presentation timestamp, in nanoseconds.
    pub fn timestamp(&self) -> u64 {
        unsafe { (*self.pointer).timestamp }
    }
}

/// Owned snapshot of an audio output's configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    /// Sample rate, in Hz.
    pub sample_rate: usize,
    /// Number of audio channels.
    pub channels: usize,
}

/// A handle to an OBS audio output (`audio_t`).
///
/// `AudioRef` exposes read-only inspection of the audio output's
/// sample rate and channel count. It is not reference-counted; libobs
/// owns the underlying object.
pub struct AudioRef {
    /// Pointer to the underlying `audio_t`.
    pub pointer: *mut audio_t,
}

impl AudioRef {
    /// Wraps a raw `audio_t*`.
    pub fn from_raw(pointer: *mut audio_t) -> Self {
        Self { pointer }
    }

    /// Returns a snapshot of the audio output's configuration.
    pub fn info(&self) -> AudioInfo {
        AudioInfo {
            sample_rate: self.sample_rate(),
            channels: self.channels(),
        }
    }

    /// Returns the sample rate, in Hz.
    pub fn sample_rate(&self) -> usize {
        unsafe { audio_output_get_sample_rate(self.pointer) as usize }
    }

    /// Returns the number of audio channels.
    pub fn channels(&self) -> usize {
        unsafe { audio_output_get_channels(self.pointer) }
    }
}
