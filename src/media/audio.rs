use obs_rs_sys::{
    audio_data, audio_output_get_channels, audio_output_get_sample_rate, audio_t, obs_audio_data,
};

pub struct AudioDataContext {
    pointer: *mut obs_audio_data,
}

impl AudioDataContext {
    pub fn from_raw(pointer: *mut obs_audio_data) -> Self {
        Self { pointer }
    }

    pub fn frames(&self) -> usize {
        unsafe {
            self.pointer
                .as_ref()
                .expect("Audio pointer was null!")
                .frames as usize
        }
    }

    pub fn channels(&self) -> usize {
        unsafe {
            self.pointer
                .as_ref()
                .expect("Audio pointer was null!")
                .data
                .len()
        }
    }

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

/// Output-side audio frame view (the buffer libobs hands an output for raw
/// audio consumption). Format/sample-rate come from the bound `AudioRef`.
pub struct AudioDataOutputContext {
    pointer: *mut audio_data,
}

impl AudioDataOutputContext {
    pub fn from_raw(pointer: *mut audio_data) -> Self {
        Self { pointer }
    }

    pub fn data_buffer(&self, idx: usize) -> *mut u8 {
        unsafe { (*self.pointer).data[idx] }
    }

    pub fn frames(&self) -> u32 {
        unsafe { (*self.pointer).frames }
    }

    pub fn timestamp(&self) -> u64 {
        unsafe { (*self.pointer).timestamp }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioInfo {
    pub sample_rate: usize,
    pub channels: usize,
}

pub struct AudioRef {
    pub pointer: *mut audio_t,
}

impl AudioRef {
    pub fn from_raw(pointer: *mut audio_t) -> Self {
        Self { pointer }
    }

    pub fn info(&self) -> AudioInfo {
        AudioInfo {
            sample_rate: self.sample_rate(),
            channels: self.channels(),
        }
    }

    pub fn sample_rate(&self) -> usize {
        unsafe { audio_output_get_sample_rate(self.pointer) as usize }
    }

    pub fn channels(&self) -> usize {
        unsafe { audio_output_get_channels(self.pointer) }
    }
}
