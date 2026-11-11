//! OpenXR <-> wgpu over Vulkan.
//!
//! OpenXR insists on creating the Vulkan instance and device itself (so the runtime can add the
//! extensions it needs and pick the physical device attached to the headset). wgpu normally does
//! both on its own. We reconcile them the way wgpu-hal intends: ask wgpu-hal which instance/device
//! extensions and features the requested wgpu `Features` need, let OpenXR create the objects with
//! exactly those, then hand them back to wgpu-hal with `from_raw` / `device_from_raw`.
//!
//! Drop order matters: drop the [`crate::FrameLoop`] (the XR session) and any [`EyeSwapchain`]
//! before [`XrVulkan`], because wgpu destroys the Vulkan device when it is dropped.

use crate::{Error, Result, XrContext, context::VIEW_TYPE};
use ash::vk::{self, Handle};
use openxr as xr;
use wgpu::hal::{self, api::Vulkan};

pub struct XrVulkan {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    raw_instance: vk::Instance,
    raw_physical_device: vk::PhysicalDevice,
    raw_device: vk::Device,
    queue_family: u32,
}

fn gfx<E: std::fmt::Debug>(what: &'static str) -> impl FnOnce(E) -> Error {
    move |e| Error::Graphics(format!("{what}: {e:?}"))
}

impl XrVulkan {
    /// Create the Vulkan instance/device through OpenXR and wrap them in wgpu.
    ///
    /// `desc` is the device the *game* wants (features and limits). Horizon asks for a lot, so
    /// this is the same descriptor it would pass to `request_device` on a desktop window.
    pub fn create(ctx: &XrContext, desc: &wgpu::DeviceDescriptor<'_>) -> Result<Self> {
        const VK_VERSION: u32 = vk::make_api_version(0, 1, 1, 0);
        let reqs = ctx
            .instance
            .graphics_requirements::<xr::Vulkan>(ctx.system)?;
        let want = xr::Version::new(1, 1, 0);
        if want < reqs.min_api_version_supported
            || want.major() > reqs.max_api_version_supported.major()
        {
            return Err(Error::Graphics(format!(
                "the OpenXR runtime wants Vulkan in [{}, {}.0.0), we need 1.1",
                reqs.min_api_version_supported,
                reqs.max_api_version_supported.major() + 1
            )));
        }

        // SAFETY (whole block): raw Vulkan and OpenXR FFI; each handle is created by the call
        // that the next one consumes, and ownership ends up with wgpu-hal as its docs require.
        unsafe {
            let entry = ash::Entry::load().map_err(gfx("loading Vulkan"))?;
            let flags = wgpu::InstanceFlags::default();

            let inst_exts = hal::vulkan::Instance::desired_extensions(&entry, VK_VERSION, flags)
                .map_err(gfx("instance extensions"))?;
            let inst_ext_ptrs: Vec<_> = inst_exts.iter().map(|e| e.as_ptr()).collect();
            let app = vk::ApplicationInfo::default()
                .api_version(VK_VERSION)
                .application_name(c"QRustCraft")
                .engine_name(c"Horizon");
            let info = vk::InstanceCreateInfo::default()
                .application_info(&app)
                .enabled_extension_names(&inst_ext_ptrs);
            let gipa = std::mem::transmute(entry.static_fn().get_instance_proc_addr);
            let raw = ctx
                .instance
                .create_vulkan_instance(ctx.system, gipa, &info as *const _ as *const _)?
                .map_err(|e| {
                    Error::Graphics(format!("vkCreateInstance: {:?}", vk::Result::from_raw(e)))
                })?;
            let raw_instance_handle = vk::Instance::from_raw(raw as _);
            let ash_instance = ash::Instance::load(entry.static_fn(), raw_instance_handle);

            let phd = vk::PhysicalDevice::from_raw(
                ctx.instance
                    .vulkan_graphics_device(ctx.system, raw_instance_handle.as_raw() as _)?
                    as _,
            );

            let hal_instance = hal::vulkan::Instance::from_raw(
                entry,
                ash_instance.clone(),
                VK_VERSION,
                ctx.android_sdk_version,
                None,
                inst_exts,
                flags,
                wgpu::MemoryBudgetThresholds::default(),
                false,
                None,
            )
            .map_err(gfx("wgpu-hal instance"))?;
            let exposed = hal_instance
                .expose_adapter(phd)
                .ok_or_else(|| Error::Graphics("wgpu cannot use the headset's GPU".into()))?;
            if !exposed.features.contains(desc.required_features) {
                return Err(Error::Graphics(format!(
                    "the headset's GPU lacks wgpu features: {:?}",
                    desc.required_features - exposed.features
                )));
            }

            // Device creation, by OpenXR, with exactly what wgpu needs.
            let queue_family = ash_instance
                .get_physical_device_queue_family_properties(phd)
                .iter()
                .position(|q| q.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .ok_or_else(|| Error::Graphics("no graphics queue".into()))?
                as u32;
            let exts = exposed
                .adapter
                .required_device_extensions(desc.required_features);
            let ext_ptrs: Vec<_> = exts.iter().map(|e| e.as_ptr()).collect();
            let mut phd_features = exposed
                .adapter
                .physical_device_features(&exts, desc.required_features);
            let queue_info = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family)
                .queue_priorities(&[1.0])];
            let dev_info = phd_features.add_to_device_create(
                vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queue_info)
                    .enabled_extension_names(&ext_ptrs),
            );
            let raw_dev = ctx
                .instance
                .create_vulkan_device(
                    ctx.system,
                    gipa,
                    phd.as_raw() as _,
                    &dev_info as *const _ as *const _,
                )?
                .map_err(|e| {
                    Error::Graphics(format!("vkCreateDevice: {:?}", vk::Result::from_raw(e)))
                })?;
            let raw_device_handle = vk::Device::from_raw(raw_dev as _);
            let ash_device = ash::Device::load(ash_instance.fp_v1_0(), raw_device_handle);

            let open = exposed
                .adapter
                .device_from_raw(
                    ash_device,
                    None,
                    &exts,
                    desc.required_features,
                    &desc.required_limits,
                    &desc.memory_hints,
                    queue_family,
                    0,
                )
                .map_err(gfx("wgpu-hal device"))?;

            let instance = wgpu::Instance::from_hal::<Vulkan>(hal_instance);
            let adapter = instance.create_adapter_from_hal::<Vulkan>(exposed);
            let (device, queue) = adapter
                .create_device_from_hal::<Vulkan>(open, desc)
                .map_err(gfx("wgpu device"))?;

            Ok(Self {
                instance,
                adapter,
                device,
                queue,
                raw_instance: raw_instance_handle,
                raw_physical_device: phd,
                raw_device: raw_device_handle,
                queue_family,
            })
        }
    }

    /// Create the OpenXR session on this device.
    pub fn create_session(
        &self,
        ctx: &XrContext,
    ) -> Result<(
        xr::Session<xr::Vulkan>,
        xr::FrameWaiter,
        xr::FrameStream<xr::Vulkan>,
    )> {
        // SAFETY: the handles are the live ones OpenXR itself created above.
        Ok(unsafe {
            ctx.instance.create_session::<xr::Vulkan>(
                ctx.system,
                &xr::vulkan::SessionCreateInfo {
                    instance: self.raw_instance.as_raw() as _,
                    physical_device: self.raw_physical_device.as_raw() as _,
                    device: self.raw_device.as_raw() as _,
                    queue_family_index: self.queue_family,
                    queue_index: 0,
                },
            )?
        })
    }
}

/// A stereo swapchain: one image with two array layers (layer 0 = left eye, 1 = right), exposed
/// to wgpu as textures. Render each eye into `view(image, eye)`.
pub struct EyeSwapchain {
    pub handle: xr::Swapchain<xr::Vulkan>,
    pub textures: Vec<wgpu::Texture>,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
}

impl EyeSwapchain {
    pub const VK_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    pub fn new(vk: &XrVulkan, session: &xr::Session<xr::Vulkan>, size: (u32, u32)) -> Result<Self> {
        let handle = session.create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | xr::SwapchainUsageFlags::SAMPLED,
            format: Self::VK_FORMAT.as_raw() as _,
            sample_count: 1,
            width: size.0,
            height: size.1,
            face_count: 1,
            array_size: 2,
            mip_count: 1,
        })?;
        let desc = wgpu::TextureDescriptor {
            label: Some("xr eye swapchain"),
            size: wgpu::Extent3d {
                width: size.0,
                height: size.1,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let hal_desc = hal::TextureDescriptor {
            label: Some("xr eye swapchain"),
            size: desc.size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::FORMAT,
            usage: wgpu::TextureUses::COLOR_TARGET | wgpu::TextureUses::RESOURCE,
            memory_flags: hal::MemoryFlags::empty(),
            view_formats: vec![],
        };
        let mut textures = vec![];
        for raw in handle.enumerate_images()? {
            // SAFETY: the image belongs to the OpenXR swapchain, which outlives the wgpu texture
            // (both live in this struct; the drop callback keeps wgpu from freeing the image).
            let tex = unsafe {
                let hal_tex = vk
                    .device
                    .as_hal::<Vulkan>()
                    .ok_or_else(|| Error::Graphics("device is not Vulkan".into()))?
                    .texture_from_raw(
                        vk::Image::from_raw(raw),
                        &hal_desc,
                        Some(Box::new(|| {})),
                        hal::vulkan::TextureMemory::External,
                    );
                vk.device.create_texture_from_hal::<Vulkan>(
                    hal_tex,
                    &desc,
                    wgpu::TextureUses::UNINITIALIZED,
                )
            };
            textures.push(tex);
        }
        Ok(Self {
            handle,
            textures,
            size,
            format: Self::FORMAT,
        })
    }

    /// Acquire and wait for the next image. Render into it, then call `release`.
    pub fn acquire(&mut self) -> Result<usize> {
        let i = self.handle.acquire_image()?;
        self.handle.wait_image(xr::Duration::INFINITE)?;
        Ok(i as usize)
    }

    pub fn release(&mut self) -> Result<()> {
        Ok(self.handle.release_image()?)
    }

    /// One eye's render-target view of image `i`.
    pub fn view(&self, i: usize, eye: usize) -> wgpu::TextureView {
        self.textures[i].create_view(&wgpu::TextureViewDescriptor {
            label: Some(if eye == 0 { "left eye" } else { "right eye" }),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: eye as u32,
            array_layer_count: Some(1),
            ..Default::default()
        })
    }

    /// The composition-layer view for an eye, given the located view.
    pub fn layer_view<'a>(
        &'a self,
        eye: usize,
        pose: xr::Posef,
        fov: xr::Fovf,
    ) -> xr::CompositionLayerProjectionView<'a, xr::Vulkan> {
        xr::CompositionLayerProjectionView::new()
            .pose(pose)
            .fov(fov)
            .sub_image(
                xr::SwapchainSubImage::new()
                    .swapchain(&self.handle)
                    .image_array_index(eye as u32)
                    .image_rect(xr::Rect2Di {
                        offset: xr::Offset2Di { x: 0, y: 0 },
                        extent: xr::Extent2Di {
                            width: self.size.0 as _,
                            height: self.size.1 as _,
                        },
                    }),
            )
    }
}

#[allow(dead_code)]
const _: xr::ViewConfigurationType = VIEW_TYPE;
