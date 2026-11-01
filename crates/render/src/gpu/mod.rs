pub mod alloc;
pub mod buffer;
pub mod command_buffer;
pub mod command_list;
pub mod descriptor;
pub mod device_iterator;
pub mod fence;
pub mod frame;
pub mod global_state;
pub mod magic_textures;
pub mod pipeline_cache;
pub mod profiler;
pub mod render_target;

use std::{
    any::Any,
    backtrace::{Backtrace, BacktraceStatus},
    ffi::{CStr, c_void},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use ahash::HashMap;
use anyhow::{Context, bail};
use ash::{
    ext::{debug_utils, descriptor_buffer, memory_budget},
    khr::{surface, swapchain},
    vk::{
        self, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
        DebugUtilsMessengerCallbackDataEXT, QueueFlags,
    },
};
use gpu_allocator::{
    MemoryLocation,
    vulkan::{Allocator, AllocatorCreateDesc},
};
use parking_lot::{Mutex, MutexGuard, RwLock};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tracing::info;

use crate::{
    gpu::{
        alloc::{ImageAllocDesc, image::Image},
        command_buffer::CommandBuffer,
        descriptor::DescriptorSizes,
        fence::GpuFence,
        frame::FrameContext,
        magic_textures::MagicTextureContainer,
        pipeline_cache::PipelineCache,
        profiler::ScopeGuard,
    },
    tfx::{d3d12_sampler::D3D12SamplerDesc, externs::BaseExternSource},
};

/// Result of presenting a swapchain image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentStatus {
    Ok,
    /// The swapchain still works, but recreating it would be preferable.
    Suboptimal,
    /// The swapchain must be recreated before rendering again.
    OutOfDate,
}

/// Swapchain resources that are replaced when the window is resized.
pub struct SwapchainState {
    pub swapchain: vk::SwapchainKHR,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub extent: vk::Extent2D,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
}

pub struct Gpu {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub swapchain_loader: swapchain::Device,
    pub surface_loader: surface::Instance,
    pub descriptor_buffer: descriptor_buffer::Device,
    #[cfg(debug_assertions)]
    pub debug_loader: debug_utils::Device,

    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,

    pub queue: vk::Queue,
    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub swapchain_state: RwLock<SwapchainState>,

    allocator: Mutex<Allocator>,

    image_available_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,

    pub command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    upload_command_pool: vk::CommandPool,
    // upload_command_buffers: Vec<Mutex<vk::CommandBuffer>>,
    upload_command_buffer: Mutex<vk::CommandBuffer>,
    upload_command_buffer_fence: vk::Fence,
    upload_command_buffer_used: AtomicBool,

    // // TODO(cohae): This (more or less) belongs in renderer
    // pub pipeline_cache: Mutex<PipelineCache>,
    pub(crate) frames: [FrameContext; Self::MAX_FRAMES_IN_FLIGHT],
    pub(crate) frame_index: AtomicUsize,
    pub(crate) frame_fence: GpuFence,
    /// List of resources to be destroyed after the frame is finished.
    bin: Mutex<Vec<(Box<dyn Any + Send>, u8)>>,

    // TODO(cohae): This belongs in renderer
    pub extern_source: RwLock<BaseExternSource>,

    pub magic_textures: MagicTextureContainer,
    pub pipeline_cache: RwLock<PipelineCache>,
    pub descriptor_sizes: DescriptorSizes,
    pub default_sampler: vk::Sampler,
    null_storage_image: Mutex<Option<(Image, vk::ImageView)>>,

    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,

    // Per-frame stats
    pub num_drawcalls: AtomicUsize,
    pub num_static_instances: AtomicUsize,

    pub(crate) sampler_cache: Mutex<HashMap<D3D12SamplerDesc, vk::Sampler>>,
}

unsafe impl Sync for Gpu {}
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for Gpu {}

impl Gpu {
    pub const MAX_FRAMES_IN_FLIGHT: usize = 3;

    pub fn new<W: HasWindowHandle + HasDisplayHandle>(
        window: &W,
        resolution: (u32, u32),
    ) -> anyhow::Result<Arc<Self>> {
        let entry = unsafe { ash::Entry::load() }?;

        let version = vk::make_api_version(0, 1, 3, 0);

        if version < vk::make_api_version(0, 1, 3, 0) {
            bail!("Vulkan 1.3 or higher is required");
        }

        let mut instance_extensions =
            ash_window::enumerate_required_extensions(window.display_handle().unwrap().as_raw())?
                .to_vec();

        #[cfg(debug_assertions)]
        instance_extensions.push(vk::EXT_DEBUG_UTILS_NAME.as_ptr());

        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default()
                    .application_info(
                        &vk::ApplicationInfo::default()
                            .engine_name(c"Deimos")
                            .engine_version(1)
                            .application_name(c"Deimos")
                            .application_version(1)
                            .api_version(version),
                    )
                    .enabled_layer_names(&[
                        #[cfg(debug_assertions)]
                        c"VK_LAYER_KHRONOS_validation".as_ptr(),
                    ])
                    .enabled_extension_names(&instance_extensions),
                None,
            )
        }?;

        let surface_loader = surface::Instance::new(&entry, &instance);

        let debug_messenger = if cfg!(debug_assertions) {
            let instance_debug = ash::ext::debug_utils::Instance::new(&entry, &instance);
            Some(unsafe {
                instance_debug
                    .create_debug_utils_messenger(
                        &vk::DebugUtilsMessengerCreateInfoEXT::default()
                            .message_severity(
                                DebugUtilsMessageSeverityFlagsEXT::ERROR
                                    | DebugUtilsMessageSeverityFlagsEXT::WARNING
                                    | DebugUtilsMessageSeverityFlagsEXT::INFO,
                            )
                            .message_type(
                                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                            )
                            .pfn_user_callback(Some(debug_message_callback)),
                        None,
                    )
                    .expect("Failed to create debug utils messenger")
            })
        } else {
            None
        };

        let window_handle = window.window_handle().unwrap().as_raw();
        let display_handle = window.display_handle().unwrap().as_raw();

        let surface = unsafe {
            ash_window::create_surface(&entry, &instance, display_handle, window_handle, None)
        }?;

        let physical_devices = unsafe { instance.enumerate_physical_devices() }?;
        let physical_device = physical_devices[0];

        let props = unsafe { instance.get_physical_device_properties(physical_device) };
        if props.api_version < version {
            bail!(
                "Vulkan device does not support required Vulkan version (expected at least {}, found {})",
                vk_api_version_string(version),
                vk_api_version_string(props.api_version)
            );
        }

        tracing::info!(
            "Selected GPU: {} (Vulkan {})",
            unsafe { CStr::from_ptr(props.device_name.as_ptr()) }.to_str()?,
            vk_api_version_string(props.api_version)
        );

        let Some(graphics_queue_family_idx) =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) }
                .iter()
                .position(|p| p.queue_flags.contains(QueueFlags::GRAPHICS))
        else {
            bail!("No graphics queue family found");
        };

        let (descriptor_sizes, descriptor_offset_alignment) = {
            let mut db_props = vk::PhysicalDeviceDescriptorBufferPropertiesEXT::default();
            let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut db_props);
            unsafe {
                instance.get_physical_device_properties2(physical_device, &mut properties);
            }

            if db_props.descriptor_buffer_offset_alignment == 0 {
                bail!("VK_EXT_descriptor_buffer is not supported by the selected device");
            }

            (
                DescriptorSizes {
                    sampler: db_props.sampler_descriptor_size,
                    sampled_image: db_props.sampled_image_descriptor_size,
                    storage_image: db_props.storage_image_descriptor_size,
                    uniform_texel_buffer: db_props.uniform_texel_buffer_descriptor_size,
                    storage_texel_buffer: db_props.storage_texel_buffer_descriptor_size,
                    uniform_buffer: db_props.uniform_buffer_descriptor_size,
                    storage_buffer: db_props.storage_buffer_descriptor_size,
                },
                db_props.descriptor_buffer_offset_alignment,
            )
        };

        let device = unsafe {
            instance.create_device(
                physical_device,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&[vk::DeviceQueueCreateInfo::default()
                        .queue_family_index(graphics_queue_family_idx as u32)
                        .queue_priorities(&[0f32])])
                    .enabled_extension_names(&[
                        swapchain::NAME.as_ptr(),
                        descriptor_buffer::NAME.as_ptr(),
                        memory_budget::NAME.as_ptr(),
                    ])
                    .enabled_features(
                        &vk::PhysicalDeviceFeatures::default()
                            .sampler_anisotropy(true)
                            .depth_clamp(true)
                            .depth_bias_clamp(true)
                            .fill_mode_non_solid(true)
                            .independent_blend(true)
                            .dual_src_blend(true)
                            .logic_op(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceVulkan11Features::default()
                            .shader_draw_parameters(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceVulkan12Features::default()
                            .timeline_semaphore(true)
                            .buffer_device_address(true)
                            .host_query_reset(true)
                            .vulkan_memory_model(true)
                            .vulkan_memory_model_device_scope(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceVulkan13Features::default()
                            .dynamic_rendering(true)
                            .shader_demote_to_helper_invocation(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default()
                            .extended_dynamic_state(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceDescriptorBufferFeaturesEXT::default()
                            .descriptor_buffer(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceDynamicRenderingLocalReadFeaturesKHR::default()
                            .dynamic_rendering_local_read(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDevicePrimitiveTopologyListRestartFeaturesEXT::default()
                            .primitive_topology_list_restart(true),
                    ),
                None,
            )
        }?;

        let default_sampler = unsafe {
            device.create_sampler(
                &vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
                    .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
                    .max_lod(0.0),
                None,
            )
        }?;

        let queue = unsafe { device.get_device_queue(graphics_queue_family_idx as u32, 0) };

        let surface_format =
            unsafe { surface_loader.get_physical_device_surface_formats(physical_device, surface) }
                .unwrap()[0];

        let descriptor_buffer = descriptor_buffer::Device::new(&instance, &device);

        let swapchain_loader = swapchain::Device::new(&instance, &device);
        let swapchain_state = create_swapchain_state(
            &device,
            &swapchain_loader,
            &surface_loader,
            physical_device,
            surface,
            surface_format,
            vk::SwapchainKHR::null(),
            Some(vk::Extent2D {
                width: resolution.0,
                height: resolution.1,
            }),
        )?;

        let image_available_semaphores = (0..Self::MAX_FRAMES_IN_FLIGHT)
            .map(|_| unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) })
            .collect::<Result<Vec<_>, _>>()?;

        let in_flight_fences = (0..Self::MAX_FRAMES_IN_FLIGHT)
            .map(|_| unsafe {
                device.create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        info!(
            "Configured surface with format {:?} at {}x{}",
            surface_format.format, swapchain_state.extent.width, swapchain_state.extent.height
        );

        let command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(graphics_queue_family_idx as u32)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }?;

        let command_buffers = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(Self::MAX_FRAMES_IN_FLIGHT as u32),
            )
        }?;

        let upload_command_pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default()
                    .queue_family_index(graphics_queue_family_idx as u32)
                    .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                None,
            )
        }?;

        let upload_command_buffer: Mutex<vk::CommandBuffer> = unsafe {
            device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(upload_command_pool)
                    .level(vk::CommandBufferLevel::PRIMARY)
                    .command_buffer_count(1),
            )
        }?[0]
            .into();
        {
            let cmd = upload_command_buffer.lock();
            unsafe {
                device.begin_command_buffer(*cmd, &vk::CommandBufferBeginInfo::default())?;
            }
        }

        let upload_command_buffer_fence =
            unsafe { device.create_fence(&vk::FenceCreateInfo::default(), None)? };

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: true,
            // cohae: Keeping device_memblock_size to 16MB fixes an VK_ERROR_OUT_OF_DEVICE_MEMORY error when replaying captures in renderdoc
            allocation_sizes: gpu_allocator::AllocationSizes::new(
                16 * 1024 * 1024,
                64 * 1024 * 1024,
            ),
        })?;

        let memory_types =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };
        let upload_memory_type_index = memory_types
            .memory_types
            .iter()
            .enumerate()
            .find(|(_, memory_type)| {
                memory_type.property_flags.contains(
                    vk::MemoryPropertyFlags::HOST_VISIBLE
                        | vk::MemoryPropertyFlags::HOST_COHERENT
                        | vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )
            })
            .map(|(index, _)| index)
            .context("No suitable upload memory type found")?
            as u32;

        info!(
            "Selected upload memory type {} for upload ring ({:?})",
            upload_memory_type_index,
            memory_types.memory_types[upload_memory_type_index as usize].property_flags
        );

        Ok(Self {
            pipeline_cache: RwLock::new(PipelineCache::new(
                &instance,
                device.clone(),
                descriptor_buffer.clone(),
            )),
            #[cfg(debug_assertions)]
            debug_loader: debug_utils::Device::new(&instance, &device),
            entry,
            instance,
            surface_loader,
            swapchain_loader,
            physical_device,
            descriptor_buffer,
            device: device.clone(),

            queue,
            surface,
            surface_format,
            swapchain_state: RwLock::new(swapchain_state),
            image_available_semaphores,

            allocator: allocator.into(),

            in_flight_fences,

            command_pool,
            command_buffers,
            upload_command_pool,
            upload_command_buffer,
            upload_command_buffer_used: AtomicBool::new(false),
            upload_command_buffer_fence,

            frames: std::array::from_fn(|_| {
                FrameContext::new(
                    device.clone(),
                    upload_memory_type_index,
                    descriptor_offset_alignment,
                    graphics_queue_family_idx as u32,
                    rayon::current_num_threads() + 1,
                    props.limits.timestamp_period,
                )
                .expect("Failed to create frame context")
            }),
            frame_index: AtomicUsize::new(0),
            frame_fence: GpuFence::new(&device)?,

            bin: Mutex::new(Vec::new()),

            extern_source: RwLock::new(BaseExternSource::None),
            magic_textures: MagicTextureContainer::default(),
            descriptor_sizes,
            default_sampler,
            null_storage_image: Mutex::new(None),

            debug_messenger,

            num_drawcalls: AtomicUsize::new(0),
            num_static_instances: AtomicUsize::new(0),

            sampler_cache: Mutex::new(HashMap::default()),
        }
        .into())
    }

    pub fn swapchain_resolution(&self) -> (u32, u32) {
        let state = self.swapchain_state.read();
        (state.extent.width, state.extent.height)
    }

    pub fn swapchain_extent(&self) -> vk::Extent2D {
        self.swapchain_state.read().extent
    }

    /// Returns the swapchain image and its view for `index`.
    pub fn swapchain_image(&self, index: u32) -> (vk::Image, vk::ImageView) {
        let state = self.swapchain_state.read();
        let index = index as usize;
        (state.images[index], state.image_views[index])
    }

    pub fn swapchain_image_count(&self) -> usize {
        self.swapchain_state.read().images.len()
    }

    /// Recreates the swapchain for a new window size.
    ///
    /// Does nothing if the extent is already current.
    pub fn resize_swapchain(&self, width: u32, height: u32) -> anyhow::Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        {
            let state = self.swapchain_state.read();
            if state.extent.width == width && state.extent.height == height {
                return Ok(());
            }
        }

        self.recreate_swapchain(width, height)
    }

    /// Recreates the swapchain, its image views, and per-image present
    /// semaphores, even if the requested extent matches the current one.
    ///
    /// Waits for the GPU to go idle first, so it is safe to call from the event
    /// loop after a window resize.
    pub fn recreate_swapchain(&self, width: u32, height: u32) -> anyhow::Result<()> {
        if width == 0 || height == 0 {
            return Ok(());
        }

        // The new swapchain is created with the old one passed as oldSwapchain, so
        // all resources currently in flight must be finished first.
        unsafe { self.device.device_wait_idle()? };

        let (old_swapchain, old_views, old_semaphores) = {
            let state = self.swapchain_state.read();
            (
                state.swapchain,
                state.image_views.clone(),
                state.render_finished_semaphores.clone(),
            )
        };

        let new_state = create_swapchain_state(
            &self.device,
            &self.swapchain_loader,
            &self.surface_loader,
            self.physical_device,
            self.surface,
            self.surface_format,
            old_swapchain,
            Some(vk::Extent2D { width, height }),
        )?;

        *self.swapchain_state.write() = new_state;

        unsafe {
            for view in old_views {
                self.device.destroy_image_view(view, None);
            }
            for semaphore in old_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }
            self.swapchain_loader.destroy_swapchain(old_swapchain, None);
        }

        let extent = self.swapchain_state.read().extent;
        info!("Resized swapchain to {}x{}", extent.width, extent.height);

        Ok(())
    }

    pub fn null_storage_image_view(self: &Arc<Self>) -> vk::ImageView {
        let mut lock = self.null_storage_image.lock();
        if let Some((_, view)) = lock.as_ref() {
            return *view;
        }

        let image = self
            .alloc_image(&ImageAllocDesc {
                name: "null_storage_image",
                image_type: vk::ImageType::TYPE_2D,
                format: vk::Format::R8G8B8A8_UNORM,
                extent: vk::Extent3D {
                    width: 1,
                    height: 1,
                    depth: 1,
                },
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SampleCountFlags::TYPE_1,
                tiling: vk::ImageTiling::OPTIMAL,
                usage: vk::ImageUsageFlags::STORAGE
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_DST,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                initial_layout: vk::ImageLayout::GENERAL,
                location: MemoryLocation::GpuOnly,
            })
            .expect("Failed to create null storage image");

        let view = unsafe {
            self.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image.vk_image())
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1)
                            .level_count(1),
                    ),
                None,
            )
        }
        .expect("Failed to create null storage image view");

        *lock = Some((image, view));
        view
    }

    pub fn current_command_buffer(self: &Arc<Self>) -> CommandBuffer {
        let cmd = self.command_buffers
            [self.frame_index.load(Ordering::Relaxed) % Self::MAX_FRAMES_IN_FLIGHT];
        CommandBuffer::from_native_command_list(self, cmd)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn cmd_image_layout_barrier(
        &self,
        command_buffer: vk::CommandBuffer,
        image: vk::Image,
        aspect_mask: vk::ImageAspectFlags,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
    ) {
        self.cmd_image_layout_barrier_range(
            command_buffer,
            image,
            vk::ImageSubresourceRange::default()
                .aspect_mask(aspect_mask)
                .base_mip_level(0)
                .level_count(1)
                .base_array_layer(0)
                .layer_count(1),
            old_layout,
            new_layout,
            src_stage,
            dst_stage,
            src_access,
            dst_access,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn cmd_image_layout_barrier_range(
        &self,
        command_buffer: vk::CommandBuffer,
        image: vk::Image,
        subresource_range: vk::ImageSubresourceRange,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
    ) {
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(subresource_range)
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);

        unsafe {
            self.device.cmd_pipeline_barrier(
                command_buffer,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }
    }

    /// Acquires the next swapchain image.
    ///
    /// Returns `None` if the swapchain is out of date and needs to be recreated.
    pub fn acquire_next_image(&self) -> anyhow::Result<Option<u32>> {
        let frame = self.frame_index.load(Ordering::Relaxed) % Self::MAX_FRAMES_IN_FLIGHT;

        unsafe {
            self.device
                .wait_for_fences(&[self.in_flight_fences[frame]], true, u64::MAX)?;
        }

        let swapchain = self.swapchain_state.read().swapchain;
        let acquired = unsafe {
            self.swapchain_loader.acquire_next_image(
                swapchain,
                u64::MAX,
                self.image_available_semaphores[frame],
                vk::Fence::null(),
            )
        };

        match acquired {
            Ok((image_index, _suboptimal)) => Ok(Some(image_index)),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn upload_command_buffer(&self) -> MutexGuard<'_, vk::CommandBuffer> {
        let lock = self.upload_command_buffer.lock();
        self.upload_command_buffer_used
            .store(true, Ordering::Relaxed);
        lock
    }

    pub fn frame_index(&self) -> usize {
        self.frame_index.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn frame(&self) -> &FrameContext {
        &self.frames[self.frame_index() % Self::MAX_FRAMES_IN_FLIGHT]
    }

    pub fn previous_frame(&self) -> &FrameContext {
        let index = if self.frame_index() == 0 {
            Self::MAX_FRAMES_IN_FLIGHT - 1
        } else {
            self.frame_index() - 1
        };
        &self.frames[index]
    }

    pub fn begin_frame(&self) -> &FrameContext {
        self.num_drawcalls
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.num_static_instances
            .store(0, std::sync::atomic::Ordering::Relaxed);

        // self.swapchain.lock().wait_on_present();
        let frame_index = self.frame_index.load(std::sync::atomic::Ordering::Relaxed);
        let frame = &self.frames[frame_index % Self::MAX_FRAMES_IN_FLIGHT];

        unsafe {
            self.device
                .wait_for_fences(
                    &[self.in_flight_fences[frame_index % Self::MAX_FRAMES_IN_FLIGHT]],
                    true,
                    u64::MAX,
                )
                .expect("wait for in-flight fence");
        }
        frame
            .wait_for_completion(&self.frame_fence)
            .expect("wait for frame completion");
        frame.begin_frame(&self.device);

        let _to_bin = self
            .bin
            .lock()
            .extract_if(.., |(_res, lifetime)| {
                if *lifetime == 0 {
                    true
                } else {
                    *lifetime -= 1;
                    false
                }
            })
            .collect::<Vec<_>>();

        frame
    }

    pub fn submit(&self, image_index: u32) -> anyhow::Result<()> {
        let frame_index = self.frame_index.load(std::sync::atomic::Ordering::Relaxed)
            % Self::MAX_FRAMES_IN_FLIGHT;
        self.frame_index.store(
            (frame_index + 1) % Self::MAX_FRAMES_IN_FLIGHT,
            Ordering::Relaxed,
        );
        let frame = &self.frames[frame_index];

        // frame.stream.acquire_cmd(self).scope(|cmd| {
        //     frame.profiler.resolve_query_data(cmd);
        // });

        // frame.stream.submit(&self.queue);
        frame.signal(&self.frame_fence, &self.queue)?;
        // self.cmd_ring.advance().expect("advance command ring");

        let command_buffers = [self.command_buffers[frame_index]];
        let wait_semaphores = [self.image_available_semaphores[frame_index]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores =
            [self.swapchain_state.read().render_finished_semaphores[image_index as usize]];

        let upload_cmd = self.upload_command_buffer.lock();
        if self
            .upload_command_buffer_used
            .swap(false, Ordering::Relaxed)
        {
            unsafe {
                self.end_command_buffer(*upload_cmd)?;
                self.queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[*upload_cmd])],
                    self.upload_command_buffer_fence,
                )?;

                self.wait_for_fences(&[self.upload_command_buffer_fence], true, u64::MAX)?;
                self.reset_fences(&[self.upload_command_buffer_fence])?;

                self.device
                    .begin_command_buffer(*upload_cmd, &vk::CommandBufferBeginInfo::default())?;
            }
        }

        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&wait_stages)
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores);

        unsafe {
            self.device
                .reset_fences(&[self.in_flight_fences[frame_index]])?;
            self.device.queue_submit(
                self.queue,
                &[submit_info],
                self.in_flight_fences[frame_index],
            )?;
        }

        drop(upload_cmd);

        Ok(())
    }

    /// Presents the given image.
    pub fn present(&self, image_index: u32) -> anyhow::Result<PresentStatus> {
        let state = self.swapchain_state.read();
        let wait_semaphores = [state.render_finished_semaphores[image_index as usize]];
        let swapchains = [state.swapchain];
        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        match unsafe {
            self.swapchain_loader
                .queue_present(self.queue, &present_info)
        } {
            Ok(false) => Ok(PresentStatus::Ok),
            Ok(true) => Ok(PresentStatus::Suboptimal),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(PresentStatus::OutOfDate),
            Err(e) => Err(e.into()),
        }
    }

    /// Acquires and begins a secondary command buffer for the current frame.
    ///
    /// Returns `None` when the frame uses more secondary buffers than were allocated for it.
    pub fn acquire_secondary_command_buffer(
        self: &Arc<Self>,
        color_formats: &[vk::Format],
        depth_format: Option<vk::Format>,
    ) -> anyhow::Result<Option<CommandBuffer>> {
        let secondary = &self.frame().secondary;
        let pool_count = secondary.pool_count();
        // Each rayon worker uses its own command pool. The last pool is the fallback for threads that are not part of the pool.
        let pool_index = match rayon::current_thread_index() {
            Some(index) if index + 1 < pool_count => index,
            _ => pool_count - 1,
        };

        let Some(buffer) = secondary.acquire(pool_index)? else {
            return Ok(None);
        };

        secondary.reset_buffer(buffer)?;

        let mut inheritance_rendering = vk::CommandBufferInheritanceRenderingInfo::default()
            .color_attachment_formats(color_formats)
            .depth_attachment_format(depth_format.unwrap_or(vk::Format::UNDEFINED))
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            .view_mask(0);

        let inheritance =
            vk::CommandBufferInheritanceInfo::default().push_next(&mut inheritance_rendering);

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(
                vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE
                    | vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
            )
            .inheritance_info(&inheritance);

        secondary.begin_buffer(buffer, &begin_info)?;

        Ok(Some(CommandBuffer::from_native_command_list(self, buffer)))
    }

    pub fn end_secondary_command_buffer(&self, buffer: vk::CommandBuffer) -> anyhow::Result<()> {
        self.frame().secondary.end_buffer(buffer)
    }

    pub fn wait_idle(&self) -> anyhow::Result<()> {
        unsafe { self.device.device_wait_idle()? };
        Ok(())
    }

    pub fn profiler_scope<'a>(
        self: &'a Arc<Self>,
        cmd: &CommandBuffer,
        name: &'static str,
    ) -> ScopeGuard<'a> {
        self.frame().profiler.scope(cmd, name)
    }
}

#[allow(clippy::too_many_arguments)]
fn create_swapchain_state(
    device: &ash::Device,
    swapchain_loader: &swapchain::Device,
    surface_loader: &surface::Instance,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    surface_format: vk::SurfaceFormatKHR,
    old_swapchain: vk::SwapchainKHR,
    requested_extent: Option<vk::Extent2D>,
) -> anyhow::Result<SwapchainState> {
    let capabilities = unsafe {
        surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?
    };

    let extent = if capabilities.current_extent.width != u32::MAX {
        capabilities.current_extent
    } else {
        let requested = requested_extent.unwrap_or(capabilities.min_image_extent);
        vk::Extent2D {
            width: requested
                .width
                .clamp(
                    capabilities.min_image_extent.width,
                    capabilities.max_image_extent.width,
                )
                .max(1),
            height: requested
                .height
                .clamp(
                    capabilities.min_image_extent.height,
                    capabilities.max_image_extent.height,
                )
                .max(1),
        }
    };

    let mut desired_image_count = capabilities.min_image_count + 1;
    if capabilities.max_image_count > 0 && desired_image_count > capabilities.max_image_count {
        desired_image_count = capabilities.max_image_count;
    }

    let pre_transform = if capabilities
        .supported_transforms
        .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
    {
        vk::SurfaceTransformFlagsKHR::IDENTITY
    } else {
        capabilities.current_transform
    };

    let present_modes = unsafe {
        surface_loader.get_physical_device_surface_present_modes(physical_device, surface)?
    };
    let present_mode = present_modes
        .iter()
        .cloned()
        .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
        .unwrap_or(vk::PresentModeKHR::FIFO);

    let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
        .surface(surface)
        .min_image_count(desired_image_count)
        .image_color_space(surface_format.color_space)
        .image_format(surface_format.format)
        .image_extent(extent)
        .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
        .pre_transform(pre_transform)
        .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
        .present_mode(present_mode)
        .clipped(true)
        .image_array_layers(1)
        .old_swapchain(old_swapchain);

    let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };

    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
    let image_views = images
        .iter()
        .map(|image| unsafe {
            device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(*image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .subresource_range(
                        vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .layer_count(1)
                            .level_count(1),
                    ),
                None,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let render_finished_semaphores = (0..images.len())
        .map(|_| unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SwapchainState {
        swapchain,
        images,
        image_views,
        extent,
        render_finished_semaphores,
    })
}

impl std::ops::Deref for Gpu {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}

pub fn vk_api_version_string(version: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(version),
        vk::api_version_minor(version),
        vk::api_version_patch(version)
    )
}

unsafe extern "system" fn debug_message_callback(
    message_severity: DebugUtilsMessageSeverityFlagsEXT,
    message_types: DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const DebugUtilsMessengerCallbackDataEXT<'_>,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    static MESSAGE_LOCK: Mutex<()> = Mutex::new(());
    let _lock = MESSAGE_LOCK.lock();

    let message = unsafe { CStr::from_ptr((*p_callback_data).p_message) }
        .to_str()
        .unwrap_or("<invalid UTF-8>");

    let mut msg_type = Vec::new();

    if message_types.contains(DebugUtilsMessageTypeFlagsEXT::GENERAL) {
        msg_type.push("GENERAL");
    }
    if message_types.contains(DebugUtilsMessageTypeFlagsEXT::VALIDATION) {
        msg_type.push("VALIDATION");
    }
    if message_types.contains(DebugUtilsMessageTypeFlagsEXT::PERFORMANCE) {
        msg_type.push("PERFORMANCE");
    }

    let message_type = msg_type.join(" | ");

    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unknown>");
    match message_severity {
        DebugUtilsMessageSeverityFlagsEXT::ERROR => {
            tracing::error!(target: "vk", "<{thread_name}> [{}] {}",  message_type, message);
        }
        DebugUtilsMessageSeverityFlagsEXT::WARNING => {
            tracing::warn!(target: "vk", "<{thread_name}> [{}] {}",  message_type, message);
        }
        DebugUtilsMessageSeverityFlagsEXT::INFO => {
            tracing::info!(target: "vk", "<{thread_name}> [{}] {}",  message_type, message);
        }
        DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
            tracing::debug!(target: "vk", "<{thread_name}> [{}] {}",  message_type, message);
        }
        _ => tracing::error!(target: "vk", "<{thread_name}> [{}] {}",  message_type, message),
    }

    if message_severity == DebugUtilsMessageSeverityFlagsEXT::ERROR {
        let bt = Backtrace::capture();
        if bt.status() == BacktraceStatus::Captured {
            println!("Backtrace:\n{}", bt);
        }
    }

    vk::FALSE
}
