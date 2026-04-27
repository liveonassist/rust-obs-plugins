use crate::encoder::context::EncodedPacketView;
use crate::media::{
    audio::AudioDataOutputContext,
    video::VideoDataOutputContext,
};
use crate::{prelude::DataObj, properties::Properties, string::ObsString};

use super::{CreatableOutputContext, OutputRef};
use crate::source::traits::CreateError;

pub trait Outputable: Sized {
    fn get_id() -> ObsString;
    fn create(
        context: &mut CreatableOutputContext<'_, Self>,
        output: OutputRef,
    ) -> Result<Self, CreateError>;

    fn start(&mut self) -> bool {
        true
    }
    fn stop(&mut self, _ts: u64) {}
}

pub trait GetNameOutput {
    fn get_name() -> ObsString;
}

pub trait RawVideoOutput: Sized {
    fn raw_video(&mut self, frame: &mut VideoDataOutputContext);
}

pub trait RawAudioOutput: Sized {
    fn raw_audio(&mut self, frame: &mut AudioDataOutputContext);
}

pub trait RawAudio2Output: Sized {
    fn raw_audio2(&mut self, idx: usize, frame: &mut AudioDataOutputContext);
}

pub trait EncodedPacketOutput: Sized {
    fn encoded_packet(&mut self, packet: &EncodedPacketView<'_>);
}

pub trait UpdateOutput: Sized {
    fn update(&mut self, settings: &mut DataObj);
}

pub trait GetDefaultsOutput {
    fn get_defaults(settings: &mut DataObj);
}

pub trait GetPropertiesOutput: Sized {
    fn get_properties(&self) -> Properties;
}

macro_rules! simple_trait_ref {
    ($($f:ident$(($($params:tt)*))? => $t:ident $(-> $ret:ty)?)*) => ($(
        pub trait $t: Sized {
            fn $f(&self $(, $($params)*)?) $(-> $ret)?;
        }
    )*)
}

simple_trait_ref! {
    get_total_bytes => GetTotalBytesOutput -> u64
    get_dropped_frames => GetDroppedFramesOutput-> i32
    get_congestion => GetCongestionOutput -> f32
    get_connect_time_ms => GetConnectTimeMsOutput -> i32
}
