use super::{CreatableOutputContext, OutputRef, traits::*};
use crate::hotkey::{Hotkey, HotkeyCallbacks};
use crate::string::DisplayExt as _;
use crate::{data::DataObj, wrapper::PtrWrapper};
use obs_rs_sys::{
    audio_data, encoder_packet, obs_hotkey_id, obs_hotkey_register_output, obs_hotkey_t,
    obs_properties, size_t, video_data,
};
use paste::item;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::forget;
use std::os::raw::{c_char, c_int};

use obs_rs_sys::{obs_data_t, obs_output_t};

struct DataWrapper<D> {
    data: D,
    #[allow(clippy::type_complexity)]
    hotkey_callbacks: HashMap<obs_hotkey_id, Box<dyn FnMut(&mut Hotkey, &mut D)>>,
}

impl<D> DataWrapper<D> {
    pub(crate) unsafe fn register_callbacks(
        &mut self,
        callbacks: HotkeyCallbacks<D>,
        output: *mut obs_output_t,
        data: *mut c_void,
    ) {
        for (name, description, func) in callbacks.into_iter() {
            let id = obs_hotkey_register_output(
                output,
                name.as_ptr(),
                description.as_ptr(),
                Some(hotkey_callback::<D>),
                data,
            );

            self.hotkey_callbacks.insert(id, func);
        }
    }
}

impl<D> From<D> for DataWrapper<D> {
    fn from(data: D) -> Self {
        DataWrapper {
            data,
            hotkey_callbacks: HashMap::new(),
        }
    }
}

pub unsafe extern "C" fn create<D: Outputable>(
    settings: *mut obs_data_t,
    output: *mut obs_output_t,
) -> *mut c_void {
    // this is later forgotten
    let Some(settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!(
            "obs handed null settings to output::create for `{}`; aborting create",
            D::get_id().display()
        );
        return std::ptr::null_mut();
    };
    let mut context = CreatableOutputContext::from_raw(settings);
    let Some(output_context) = OutputRef::from_raw(output) else {
        log::error!(
            "obs handed null obs_output_t to output::create for `{}`; aborting create",
            D::get_id().display()
        );
        forget(context.settings);
        return std::ptr::null_mut();
    };

    let data = match D::create(&mut context, output_context) {
        Ok(data) => data,
        Err(e) => {
            log::error!("output::create for `{}` failed: {}", D::get_id().display(), e);
            forget(context.settings);
            return std::ptr::null_mut();
        }
    };
    let wrapper = Box::new(DataWrapper::from(data));
    forget(context.settings);
    let callbacks = context.hotkey_callbacks;

    let pointer = Box::into_raw(wrapper);

    // SAFETY: `pointer` came from `Box::into_raw` and is non-null.
    (*pointer).register_callbacks(callbacks, output, pointer as *mut c_void);

    pointer as *mut c_void
}

pub unsafe extern "C" fn destroy<D>(data: *mut c_void) {
    let wrapper: Box<DataWrapper<D>> = Box::from_raw(data as *mut DataWrapper<D>);
    drop(wrapper);
}

macro_rules! impl_simple_fn_mut {
    ($($name:ident$(($($params_name:tt:$params_ty:ty),*))? => $trait:ident $(-> $ret:ty)?)*) => ($(
        item! {
            pub unsafe extern "C" fn $name<D: $trait>(
                data: *mut ::std::os::raw::c_void,
                $($($params_name:$params_ty),*)?
            ) $(-> $ret)? {
                let wrapper = &mut *(data as *mut DataWrapper<D>);
                D::$name(&mut wrapper.data $(,$($params_name),*)?)
            }
        }
    )*)
}

macro_rules! impl_simple_fn_ref {
    ($($name:ident$(($($params_name:tt:$params_ty:ty),*))? => $trait:ident $(-> $ret:ty)?)*) => ($(
        item! {
            pub unsafe extern "C" fn $name<D: $trait>(
                data: *mut ::std::os::raw::c_void,
                $($($params_name:$params_ty),*)?
            ) $(-> $ret)? {
                let wrapper = &*(data as *const DataWrapper<D>);
                D::$name(&wrapper.data $(,$($params_name),*)?)
            }
        }
    )*)
}

pub unsafe extern "C" fn get_name<D: GetNameOutput>(_type_data: *mut c_void) -> *const c_char {
    D::get_name().as_ptr()
}

impl_simple_fn_mut! {
    start => Outputable -> bool
    stop(ts: u64) => Outputable
}

pub unsafe extern "C" fn raw_video<D: RawVideoOutput>(data: *mut c_void, frame: *mut video_data) {
    let wrapper = &mut *(data as *mut DataWrapper<D>);
    let mut ctx = crate::media::video::VideoDataOutputContext::from_raw(frame);
    D::raw_video(&mut wrapper.data, &mut ctx)
}

pub unsafe extern "C" fn raw_audio<D: RawAudioOutput>(data: *mut c_void, frame: *mut audio_data) {
    let wrapper = &mut *(data as *mut DataWrapper<D>);
    let mut ctx = crate::media::audio::AudioDataOutputContext::from_raw(frame);
    D::raw_audio(&mut wrapper.data, &mut ctx)
}

pub unsafe extern "C" fn raw_audio2<D: RawAudio2Output>(
    data: *mut c_void,
    idx: size_t,
    frame: *mut audio_data,
) {
    let wrapper = &mut *(data as *mut DataWrapper<D>);
    let mut ctx = crate::media::audio::AudioDataOutputContext::from_raw(frame);
    D::raw_audio2(&mut wrapper.data, idx, &mut ctx)
}

pub unsafe extern "C" fn encoded_packet<D: EncodedPacketOutput>(
    data: *mut c_void,
    packet: *mut encoder_packet,
) {
    let wrapper = &mut *(data as *mut DataWrapper<D>);
    let view = crate::encoder::context::EncodedPacketView::from_raw(&*packet);
    D::encoded_packet(&mut wrapper.data, &view)
}

pub unsafe extern "C" fn update<D: UpdateOutput>(data: *mut c_void, settings: *mut obs_data_t) {
    let data: &mut DataWrapper<D> = &mut *(data as *mut DataWrapper<D>);
    // this is later forgotten
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to output::update; skipping");
        return;
    };
    D::update(&mut data.data, &mut settings);
    forget(settings);
}

pub unsafe extern "C" fn get_defaults<D: GetDefaultsOutput>(settings: *mut obs_data_t) {
    // this is later forgotten
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to output::get_defaults; skipping");
        return;
    };
    D::get_defaults(&mut settings);
    forget(settings);
}

// pub unsafe extern "C" fn get_defaults2<D: GetDefaults2Output>(
//     data: *mut c_void,
//     settings: *mut obs_data_t,
// ) {
//     let mut settings = DataObj::from_raw(settings);
//     D::get_defaults2(??, &mut settings);
//     forget(settings);
// }

pub unsafe extern "C" fn get_properties<D: GetPropertiesOutput>(
    data: *mut ::std::os::raw::c_void,
) -> *mut obs_properties {
    let wrapper: &DataWrapper<D> = &*(data as *const DataWrapper<D>);
    let properties = D::get_properties(&wrapper.data);
    properties.into_raw()
}

impl_simple_fn_ref! {
    get_total_bytes => GetTotalBytesOutput -> u64
    get_dropped_frames => GetDroppedFramesOutput-> c_int
    get_congestion => GetCongestionOutput -> f32
    get_connect_time_ms => GetConnectTimeMsOutput -> c_int
}

pub unsafe extern "C" fn hotkey_callback<D>(
    data: *mut c_void,
    id: obs_hotkey_id,
    hotkey: *mut obs_hotkey_t,
    pressed: bool,
) {
    let wrapper: &mut DataWrapper<D> = &mut *(data as *mut DataWrapper<D>);

    let data = &mut wrapper.data;
    let hotkey_callbacks = &mut wrapper.hotkey_callbacks;
    let mut key = Hotkey::from_raw(hotkey, pressed);

    if let Some(callback) = hotkey_callbacks.get_mut(&id) {
        callback(&mut key, data);
    }
}
