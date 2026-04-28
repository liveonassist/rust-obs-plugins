use std::ffi::c_void;
use std::mem::forget;
use std::os::raw::c_char;

use obs_sys_rs::{encoder_frame, encoder_packet, obs_data_t, obs_encoder_t, obs_properties};

use crate::data::DataObj;
use crate::wrapper::PtrWrapper;

use super::EncoderRef;
use super::context::{CreatableEncoderContext, EncodeStatus, EncoderFrame, EncoderPacket};
use super::traits::*;

/// Per-instance state we attach to each encoder. Holds the user's encoder
/// data plus a buffer we point `encoder_packet.data` at across calls.
pub(crate) struct EncoderWrapper<D> {
    pub(crate) data: D,
    pub(crate) buffer: Vec<u8>,
    pub(crate) extra_data: Vec<u8>,
    pub(crate) sei_data: Vec<u8>,
}

impl<D> EncoderWrapper<D> {
    fn new(data: D) -> Self {
        Self {
            data,
            buffer: Vec::new(),
            extra_data: Vec::new(),
            sei_data: Vec::new(),
        }
    }
}

pub unsafe extern "C" fn get_name<D: GetNameEncoder>(_type_data: *mut c_void) -> *const c_char {
    D::get_name().as_ptr()
}

pub unsafe extern "C" fn create<D: Encodable>(
    settings: *mut obs_data_t,
    encoder: *mut obs_encoder_t,
) -> *mut c_void {
    let Some(settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!(
            "obs handed null settings to encoder::create for `{}`; aborting create",
            D::get_id().to_string_lossy()
        );
        return std::ptr::null_mut();
    };
    let Some(encoder_ref) = EncoderRef::from_raw(encoder) else {
        log::error!(
            "obs handed null obs_encoder_t to encoder::create for `{}`; aborting create",
            D::get_id().to_string_lossy()
        );
        forget(settings);
        return std::ptr::null_mut();
    };
    let mut ctx = CreatableEncoderContext::<D>::new(settings);
    let data = match D::create(&mut ctx, encoder_ref) {
        Ok(d) => d,
        Err(e) => {
            log::error!(
                "encoder::create for `{}` failed: {}",
                D::get_id().to_string_lossy(),
                e
            );
            forget(ctx.settings);
            return std::ptr::null_mut();
        }
    };
    forget(ctx.settings);
    let wrapper = Box::new(EncoderWrapper::new(data));
    Box::into_raw(wrapper) as *mut c_void
}

pub unsafe extern "C" fn destroy<D>(data: *mut c_void) {
    let _: Box<EncoderWrapper<D>> = Box::from_raw(data as *mut EncoderWrapper<D>);
}

pub unsafe extern "C" fn encode<D: EncodeEncoder>(
    data: *mut c_void,
    frame: *mut encoder_frame,
    packet: *mut encoder_packet,
    received_packet: *mut bool,
) -> bool {
    let wrapper = &mut *(data as *mut EncoderWrapper<D>);
    let frame_ref = EncoderFrame::from_raw(&*frame);
    let pkt_raw = &mut *packet;
    let mut pkt = EncoderPacket {
        raw: pkt_raw,
        buffer: &mut wrapper.buffer,
    };
    pkt.reset();
    match D::encode(&mut wrapper.data, &frame_ref, &mut pkt) {
        Ok(EncodeStatus::Received) => {
            // Wire the packet payload pointer to our owned buffer.
            pkt_raw.data = wrapper.buffer.as_mut_ptr();
            pkt_raw.size = wrapper.buffer.len();
            *received_packet = true;
            true
        }
        Ok(EncodeStatus::NotReady) => {
            *received_packet = false;
            true
        }
        Err(e) => {
            log::error!("encoder::encode failed: {}", e);
            *received_packet = false;
            false
        }
    }
}

pub unsafe extern "C" fn encode_texture<D: EncodeTextureEncoder>(
    data: *mut c_void,
    handle: u32,
    pts: i64,
    lock_key: u64,
    next_key: *mut u64,
    packet: *mut encoder_packet,
    received_packet: *mut bool,
) -> bool {
    let wrapper = &mut *(data as *mut EncoderWrapper<D>);
    let pkt_raw = &mut *packet;
    let mut pkt = EncoderPacket {
        raw: pkt_raw,
        buffer: &mut wrapper.buffer,
    };
    pkt.reset();
    match D::encode_texture(
        &mut wrapper.data,
        handle,
        pts,
        lock_key,
        &mut *next_key,
        &mut pkt,
    ) {
        Ok(EncodeStatus::Received) => {
            pkt_raw.data = wrapper.buffer.as_mut_ptr();
            pkt_raw.size = wrapper.buffer.len();
            *received_packet = true;
            true
        }
        Ok(EncodeStatus::NotReady) => {
            *received_packet = false;
            true
        }
        Err(e) => {
            log::error!("encoder::encode_texture failed: {}", e);
            *received_packet = false;
            false
        }
    }
}

pub unsafe extern "C" fn update<D: UpdateEncoder>(
    data: *mut c_void,
    settings: *mut obs_data_t,
) -> bool {
    let wrapper = &mut *(data as *mut EncoderWrapper<D>);
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to encoder::update; skipping");
        return false;
    };
    let ok = D::update(&mut wrapper.data, &mut settings);
    forget(settings);
    ok
}

pub unsafe extern "C" fn get_defaults<D: GetDefaultsEncoder>(settings: *mut obs_data_t) {
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to encoder::get_defaults; skipping");
        return;
    };
    D::get_defaults(&mut settings);
    forget(settings);
}

pub unsafe extern "C" fn get_properties<D: GetPropertiesEncoder>(
    data: *mut c_void,
) -> *mut obs_properties {
    let wrapper = &*(data as *const EncoderWrapper<D>);
    D::get_properties(&wrapper.data).into_raw()
}

pub unsafe extern "C" fn get_extra_data<D: GetExtraDataEncoder>(
    data: *mut c_void,
    extra_data: *mut *mut u8,
    size: *mut usize,
) -> bool {
    let wrapper = &mut *(data as *mut EncoderWrapper<D>);
    wrapper.extra_data.clear();
    if D::get_extra_data(&mut wrapper.data, &mut wrapper.extra_data) {
        *extra_data = wrapper.extra_data.as_mut_ptr();
        *size = wrapper.extra_data.len();
        true
    } else {
        false
    }
}

pub unsafe extern "C" fn get_sei_data<D: GetSeiDataEncoder>(
    data: *mut c_void,
    sei_data: *mut *mut u8,
    size: *mut usize,
) -> bool {
    let wrapper = &mut *(data as *mut EncoderWrapper<D>);
    wrapper.sei_data.clear();
    if D::get_sei_data(&mut wrapper.data, &mut wrapper.sei_data) {
        *sei_data = wrapper.sei_data.as_mut_ptr();
        *size = wrapper.sei_data.len();
        true
    } else {
        false
    }
}

pub unsafe extern "C" fn get_frame_size<D: GetFrameSizeEncoder>(data: *mut c_void) -> usize {
    let wrapper = &*(data as *const EncoderWrapper<D>);
    D::get_frame_size(&wrapper.data)
}
