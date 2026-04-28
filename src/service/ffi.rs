use std::ffi::{CString, c_void};
use std::mem::forget;
use std::os::raw::{c_char, c_int};
use std::ptr;

use obs_sys_rs::{
    obs_data_t, obs_output_t, obs_properties_t, obs_service_connect_info, obs_service_resolution,
    obs_service_t,
};

use super::context::{ConnectInfo, CreatableServiceContext, ServiceRef};
use super::traits::*;
use crate::data::DataObj;
use crate::output::OutputRef;
use crate::string::ptr_or_null;
use crate::wrapper::PtrWrapper;

/// Per-instance state attached to each service handle. Holds the user's
/// service data plus scratch buffers for callbacks that have to hand
/// libobs a stable pointer (codec lists, resolutions).
pub(crate) struct ServiceWrapper<D> {
    pub(crate) data: D,
    pub(crate) video_codec_strings: Vec<CString>,
    pub(crate) video_codec_ptrs: Vec<*const c_char>,
    pub(crate) audio_codec_strings: Vec<CString>,
    pub(crate) audio_codec_ptrs: Vec<*const c_char>,
    pub(crate) resolutions: Vec<obs_service_resolution>,
}

impl<D> ServiceWrapper<D> {
    fn new(data: D) -> Self {
        Self {
            data,
            video_codec_strings: Vec::new(),
            video_codec_ptrs: Vec::new(),
            audio_codec_strings: Vec::new(),
            audio_codec_ptrs: Vec::new(),
            resolutions: Vec::new(),
        }
    }
}

pub unsafe extern "C" fn get_name<D: GetNameService>(_type_data: *mut c_void) -> *const c_char {
    D::get_name().as_ptr()
}

pub unsafe extern "C" fn create<D: Serviceable>(
    settings: *mut obs_data_t,
    service: *mut obs_service_t,
) -> *mut c_void {
    let Some(settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!(
            "obs handed null settings to service::create for `{}`; aborting create",
            D::get_id().to_string_lossy()
        );
        return ptr::null_mut();
    };
    let Some(service_ref) = ServiceRef::from_raw(service) else {
        log::error!(
            "obs handed null obs_service_t to service::create for `{}`; aborting create",
            D::get_id().to_string_lossy()
        );
        forget(settings);
        return ptr::null_mut();
    };

    let mut ctx = CreatableServiceContext::from_raw(settings);
    let data = match D::create(&mut ctx, service_ref) {
        Ok(d) => d,
        Err(e) => {
            log::error!(
                "service::create for `{}` failed: {}",
                D::get_id().to_string_lossy(),
                e
            );
            forget(ctx.settings);
            return ptr::null_mut();
        }
    };
    forget(ctx.settings);

    let wrapper = Box::new(ServiceWrapper::new(data));
    Box::into_raw(wrapper) as *mut c_void
}

pub unsafe extern "C" fn destroy<D>(data: *mut c_void) {
    let _: Box<ServiceWrapper<D>> = Box::from_raw(data as *mut ServiceWrapper<D>);
}

pub unsafe extern "C" fn activate<D: ActivateService>(
    data: *mut c_void,
    settings: *mut obs_data_t,
) {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to service::activate; skipping");
        return;
    };
    D::activate(&mut wrapper.data, &mut settings);
    forget(settings);
}

pub unsafe extern "C" fn deactivate<D: DeactivateService>(data: *mut c_void) {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    D::deactivate(&mut wrapper.data);
}

pub unsafe extern "C" fn update<D: UpdateService>(data: *mut c_void, settings: *mut obs_data_t) {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to service::update; skipping");
        return;
    };
    D::update(&mut wrapper.data, &mut settings);
    forget(settings);
}

pub unsafe extern "C" fn get_defaults<D: GetDefaultsService>(settings: *mut obs_data_t) {
    let Some(mut settings) = DataObj::from_raw_unchecked(settings) else {
        log::error!("obs handed null settings to service::get_defaults; skipping");
        return;
    };
    D::get_defaults(&mut settings);
    forget(settings);
}

pub unsafe extern "C" fn get_properties<D: GetPropertiesService>(
    data: *mut c_void,
) -> *mut obs_properties_t {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    D::get_properties(&wrapper.data).into_raw()
}

pub unsafe extern "C" fn initialize<D: InitializeService>(
    data: *mut c_void,
    output: *mut obs_output_t,
) -> bool {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let Some(mut output_ref) = OutputRef::from_raw(output) else {
        log::error!("obs handed null obs_output_t to service::initialize; aborting init");
        return false;
    };
    D::initialize(&mut wrapper.data, &mut output_ref)
}

pub unsafe extern "C" fn get_url<D: GetUrlService>(data: *mut c_void) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_url(&wrapper.data))
}

pub unsafe extern "C" fn get_key<D: GetKeyService>(data: *mut c_void) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_key(&wrapper.data))
}

pub unsafe extern "C" fn get_username<D: GetUsernameService>(data: *mut c_void) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_username(&wrapper.data))
}

pub unsafe extern "C" fn get_password<D: GetPasswordService>(data: *mut c_void) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_password(&wrapper.data))
}

pub unsafe extern "C" fn apply_encoder_settings<D: ApplyEncoderSettingsService>(
    data: *mut c_void,
    video_encoder_settings: *mut obs_data_t,
    audio_encoder_settings: *mut obs_data_t,
) {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let mut video = DataObj::from_raw_unchecked(video_encoder_settings);
    let mut audio = DataObj::from_raw_unchecked(audio_encoder_settings);
    D::apply_encoder_settings(&mut wrapper.data, video.as_mut(), audio.as_mut());
    if let Some(v) = video {
        forget(v);
    }
    if let Some(a) = audio {
        forget(a);
    }
}

pub unsafe extern "C" fn get_output_type<D: GetOutputTypeService>(
    data: *mut c_void,
) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_output_type(&wrapper.data))
}

pub unsafe extern "C" fn get_supported_resolutions<D: GetSupportedResolutionsService>(
    data: *mut c_void,
    resolutions: *mut *mut obs_service_resolution,
    count: *mut usize,
) {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let new_resolutions = D::get_supported_resolutions(&wrapper.data);
    wrapper.resolutions = new_resolutions.into_iter().map(|r| r.into_raw()).collect();
    *resolutions = wrapper.resolutions.as_mut_ptr();
    *count = wrapper.resolutions.len();
}

pub unsafe extern "C" fn get_max_fps<D: GetMaxFpsService>(data: *mut c_void, fps: *mut c_int) {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    *fps = D::get_max_fps(&wrapper.data);
}

pub unsafe extern "C" fn get_max_bitrate<D: GetMaxBitrateService>(
    data: *mut c_void,
    video_bitrate: *mut c_int,
    audio_bitrate: *mut c_int,
) {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    let (v, a) = D::get_max_bitrate(&wrapper.data);
    *video_bitrate = v;
    *audio_bitrate = a;
}

unsafe fn refill_codecs(
    strings: &mut Vec<CString>,
    ptrs: &mut Vec<*const c_char>,
    new: Vec<CString>,
) {
    *strings = new;
    ptrs.clear();
    ptrs.extend(strings.iter().map(|s| s.as_ptr()));
    // libobs walks the array until it sees a NULL terminator.
    ptrs.push(ptr::null());
}

pub unsafe extern "C" fn get_supported_video_codecs<D: GetSupportedVideoCodecsService>(
    data: *mut c_void,
) -> *mut *const c_char {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let new = D::get_supported_video_codecs(&wrapper.data);
    refill_codecs(
        &mut wrapper.video_codec_strings,
        &mut wrapper.video_codec_ptrs,
        new,
    );
    wrapper.video_codec_ptrs.as_mut_ptr()
}

pub unsafe extern "C" fn get_supported_audio_codecs<D: GetSupportedAudioCodecsService>(
    data: *mut c_void,
) -> *mut *const c_char {
    let wrapper = &mut *(data as *mut ServiceWrapper<D>);
    let new = D::get_supported_audio_codecs(&wrapper.data);
    refill_codecs(
        &mut wrapper.audio_codec_strings,
        &mut wrapper.audio_codec_ptrs,
        new,
    );
    wrapper.audio_codec_ptrs.as_mut_ptr()
}

pub unsafe extern "C" fn get_protocol<D: GetProtocolService>(data: *mut c_void) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_protocol(&wrapper.data))
}

pub unsafe extern "C" fn get_connect_info<D: GetConnectInfoService>(
    data: *mut c_void,
    type_: obs_service_connect_info,
) -> *const c_char {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    ptr_or_null(D::get_connect_info(
        &wrapper.data,
        ConnectInfo::from_raw(type_),
    ))
}

pub unsafe extern "C" fn can_try_to_connect<D: CanTryToConnectService>(data: *mut c_void) -> bool {
    let wrapper = &*(data as *const ServiceWrapper<D>);
    D::can_try_to_connect(&wrapper.data)
}
