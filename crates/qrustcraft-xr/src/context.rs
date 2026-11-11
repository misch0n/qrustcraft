//! Loading OpenXR and describing the headset.

use crate::{Error, Result};
use openxr as xr;

#[derive(Clone, Debug)]
pub struct XrOptions {
    pub app_name: String,
    pub engine_name: String,
    /// Ask for hand tracking when the runtime has it (not required).
    pub want_hand_tracking: bool,
    /// Ask for a refresh rate (Hz) when the runtime exposes the extension; Quest 3 does 72/80/90/120.
    pub preferred_refresh_hz: Option<f32>,
}

impl Default for XrOptions {
    fn default() -> Self {
        Self {
            app_name: "QRustCraft".into(),
            engine_name: "Horizon".into(),
            want_hand_tracking: false,
            preferred_refresh_hz: Some(90.0),
        }
    }
}

pub struct XrContext {
    pub entry: xr::Entry,
    pub instance: xr::Instance,
    pub system: xr::SystemId,
    pub blend_mode: xr::EnvironmentBlendMode,
    /// Per-eye recommended swapchain size.
    pub eye_size: (u32, u32),
    pub max_eye_size: (u32, u32),
    pub runtime: String,
    pub extensions: xr::ExtensionSet,
}

pub(crate) const VIEW_TYPE: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

impl XrContext {
    #[cfg(not(target_os = "android"))]
    pub fn new(opts: &XrOptions) -> Result<Self> {
        let entry = unsafe { xr::Entry::load(&()) }.map_err(|e| Error::Loader(e.to_string()))?;
        Self::with_entry(entry, opts, &())
    }

    /// Android: `vm` and `activity` come from the NativeActivity glue (`android-activity`).
    #[cfg(target_os = "android")]
    pub fn new(
        opts: &XrOptions,
        vm: *mut std::ffi::c_void,
        activity: *mut std::ffi::c_void,
    ) -> Result<Self> {
        let info = unsafe { xr::AndroidPlatformInfo::new(vm, activity) };
        let entry = xr::Entry::linked(&info).map_err(|e| Error::Loader(e.to_string()))?;
        Self::with_entry(entry, opts, &info)
    }

    fn with_entry<P: xr::PlatformInfo>(
        entry: xr::Entry,
        opts: &XrOptions,
        platform: &P,
    ) -> Result<Self> {
        let available = entry.enumerate_extensions()?;
        if !available.khr_vulkan_enable2 {
            return Err(Error::MissingExtension("XR_KHR_vulkan_enable2"));
        }
        let mut ext = xr::ExtensionSet::default();
        ext.khr_vulkan_enable2 = true;
        #[cfg(target_os = "android")]
        {
            ext.khr_android_create_instance = true;
        }
        // Optional niceties: only enabled when offered, never required.
        ext.ext_hand_tracking = opts.want_hand_tracking && available.ext_hand_tracking;
        ext.fb_display_refresh_rate =
            opts.preferred_refresh_hz.is_some() && available.fb_display_refresh_rate;

        let instance = entry.create_instance(
            &xr::ApplicationInfo {
                application_name: &opts.app_name,
                application_version: 1,
                engine_name: &opts.engine_name,
                engine_version: 1,
                api_version: xr::Version::new(1, 0, 0),
            },
            &ext,
            &[],
            platform,
        )?;
        let props = instance.properties()?;
        let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
        let blend_mode = instance.enumerate_environment_blend_modes(system, VIEW_TYPE)?[0];
        let views = instance.enumerate_view_configuration_views(system, VIEW_TYPE)?;
        let v = views
            .first()
            .ok_or(Error::Xr(xr::sys::Result::ERROR_VALIDATION_FAILURE))?;
        Ok(Self {
            entry,
            instance,
            system,
            blend_mode,
            eye_size: (
                v.recommended_image_rect_width,
                v.recommended_image_rect_height,
            ),
            max_eye_size: (v.max_image_rect_width, v.max_image_rect_height),
            runtime: format!("{} {}", props.runtime_name, props.runtime_version),
            extensions: ext,
        })
    }
}
