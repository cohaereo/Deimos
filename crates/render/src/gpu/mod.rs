pub mod alloc;
pub mod buffer;
pub mod command_buffer;
pub mod command_list;
pub mod device_iterator;
pub mod fence;
pub mod frame;
pub mod global_state;
pub mod magic_textures;
pub mod pipeline_cache;
pub mod render_target;

use std::{
    any::Any,
    backtrace::{Backtrace, BacktraceStatus},
    ffi::{CStr, c_void},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, bail};
use ash::{
    ext::{debug_utils, descriptor_buffer, memory_budget},
    khr::{dynamic_rendering_local_read, surface, swapchain},
    vk::{
        self, DebugUtilsMessageSeverityFlagsEXT, DebugUtilsMessageTypeFlagsEXT,
        DebugUtilsMessengerCallbackDataEXT, QueueFlags,
    },
};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use parking_lot::{Mutex, MutexGuard, RwLock};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tracing::info;

use crate::{
    gpu::{
        command_buffer::CommandBuffer, fence::GpuFence, frame::FrameContext,
        magic_textures::MagicTextureContainer,
    },
    tfx::externs::BaseExternSource,
};

pub struct Gpu {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub swapchain_loader: swapchain::Device,
    pub surface_loader: surface::Instance,
    pub descriptor_buffer: descriptor_buffer::Device,
    pub debug_loader: debug_utils::Device,

    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,

    pub queue: vk::Queue,
    pub surface: vk::SurfaceKHR,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<vk::Image>,
    pub swapchain_image_views: Vec<vk::ImageView>,
    pub surface_format: vk::SurfaceFormatKHR,

    allocator: Mutex<Allocator>,

    swapchain_image_layouts: Mutex<Vec<vk::ImageLayout>>,

    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
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
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    pub swapchain_extent: vk::Extent2D,

    // Per-frame stats
    pub num_drawcalls: AtomicUsize,
    pub num_static_instances: AtomicUsize,
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
                    .push_next(
                        &mut vk::PhysicalDeviceVulkan11Features::default()
                            .shader_draw_parameters(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceVulkan12Features::default().timeline_semaphore(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceVulkan13Features::default().dynamic_rendering(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceExtendedDynamicStateFeaturesEXT::default()
                            .extended_dynamic_state(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceBufferDeviceAddressFeaturesEXT::default()
                            .buffer_device_address(true),
                    )
                    .push_next(
                        &mut vk::PhysicalDeviceDynamicRenderingLocalReadFeaturesKHR::default()
                            .dynamic_rendering_local_read(true),
                    ),
                None,
            )
        }?;

        let debug_loader = debug_utils::Device::new(&instance, &device);

        let queue = unsafe { device.get_device_queue(graphics_queue_family_idx as u32, 0) };

        let surface_format =
            unsafe { surface_loader.get_physical_device_surface_formats(physical_device, surface) }
                .unwrap()[0];

        let descriptor_buffer = descriptor_buffer::Device::new(&instance, &device);

        let surface_capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)
        }
        .unwrap();
        let mut desired_image_count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count > 0
            && desired_image_count > surface_capabilities.max_image_count
        {
            desired_image_count = surface_capabilities.max_image_count;
        }
        let swapchain_extent = match surface_capabilities.current_extent.width {
            u32::MAX => vk::Extent2D {
                width: resolution.0,
                height: resolution.1,
            },
            _ => surface_capabilities.current_extent,
        };
        let pre_transform = if surface_capabilities
            .supported_transforms
            .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
        {
            vk::SurfaceTransformFlagsKHR::IDENTITY
        } else {
            surface_capabilities.current_transform
        };
        let present_modes = unsafe {
            surface_loader
                .get_physical_device_surface_present_modes(physical_device, surface)
                .unwrap()
        };
        let present_mode = present_modes
            .iter()
            .cloned()
            .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(vk::PresentModeKHR::FIFO);
        let swapchain_loader = swapchain::Device::new(&instance, &device);

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(desired_image_count)
            .image_color_space(surface_format.color_space)
            .image_format(surface_format.format)
            .image_extent(swapchain_extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(pre_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .image_array_layers(1);

        let swapchain =
            unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None) }.unwrap();

        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }?;
        let swapchain_image_views = swapchain_images
            .iter()
            .map(|img| {
                unsafe {
                    device.create_image_view(
                        &vk::ImageViewCreateInfo::default()
                            .image(*img)
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
                }
                .unwrap()
            })
            .collect();

        let image_available_semaphores = (0..Self::MAX_FRAMES_IN_FLIGHT)
            .map(|_| unsafe { device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None) })
            .collect::<Result<Vec<_>, _>>()?;

        let render_finished_semaphores = (0..swapchain_images.len())
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
            surface_format.format, swapchain_extent.width, swapchain_extent.height
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

        let swapchain_image_layouts =
            Mutex::new(vec![vk::ImageLayout::UNDEFINED; swapchain_images.len()]);

        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: device.clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: Default::default(),
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
            entry,
            instance,
            surface_loader,
            swapchain_loader,
            physical_device,
            debug_loader,
            descriptor_buffer,
            device: device.clone(),

            queue,
            surface,
            swapchain,
            swapchain_extent,
            swapchain_images,
            swapchain_image_views,
            surface_format,
            swapchain_image_layouts,
            image_available_semaphores,

            allocator: allocator.into(),

            render_finished_semaphores,
            in_flight_fences,

            command_pool,
            command_buffers,
            upload_command_pool,
            upload_command_buffer,
            upload_command_buffer_used: AtomicBool::new(false),
            upload_command_buffer_fence,

            frames: std::array::from_fn(|_| {
                FrameContext::new(device.clone(), upload_memory_type_index)
                    .expect("Failed to create frame context")
            }),
            frame_index: AtomicUsize::new(0),
            frame_fence: GpuFence::new(&device)?,

            bin: Mutex::new(Vec::new()),

            extern_source: RwLock::new(BaseExternSource::None),
            magic_textures: MagicTextureContainer::default(),

            debug_messenger,

            num_drawcalls: AtomicUsize::new(0),
            num_static_instances: AtomicUsize::new(0),
        }
        .into())
    }

    pub fn swapchain_resolution(&self) -> (u32, u32) {
        (self.swapchain_extent.width, self.swapchain_extent.height)
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

    pub fn acquire_next_image(&self) -> anyhow::Result<u32> {
        let frame = self.frame_index.load(Ordering::Relaxed) % Self::MAX_FRAMES_IN_FLIGHT;

        unsafe {
            self.device
                .wait_for_fences(&[self.in_flight_fences[frame]], true, u64::MAX)?;
        }

        let (image_index, _suboptimal) = unsafe {
            self.swapchain_loader.acquire_next_image(
                self.swapchain,
                u64::MAX,
                self.image_available_semaphores[frame],
                vk::Fence::null(),
            )
        }?;

        Ok(image_index)
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
        &self.frames[(self.frame_index() - 1) % Self::MAX_FRAMES_IN_FLIGHT]
    }

    pub fn begin_frame(&self) -> &FrameContext {
        self.num_drawcalls
            .store(0, std::sync::atomic::Ordering::Relaxed);
        self.num_static_instances
            .store(0, std::sync::atomic::Ordering::Relaxed);

        // self.swapchain.lock().wait_on_present();
        let frame_index = self.frame_index.load(std::sync::atomic::Ordering::Relaxed);
        let frame = &self.frames[frame_index % Self::MAX_FRAMES_IN_FLIGHT];
        frame
            .wait_for_completion(&self.frame_fence)
            .expect("wait for frame completion");
        frame.begin_frame();

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
        let next_frame_index = (frame_index + 1) % Self::MAX_FRAMES_IN_FLIGHT;
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

        self.frame_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let command_buffers = [self.command_buffers[frame_index]];
        let wait_semaphores = [self.image_available_semaphores[frame_index]];
        let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let signal_semaphores = [self.render_finished_semaphores[image_index as usize]];

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

    pub fn present(&self, image_index: u32) -> anyhow::Result<()> {
        let wait_semaphores = [self.render_finished_semaphores[image_index as usize]];
        let swapchains = [self.swapchain];
        let image_indices = [image_index];

        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        let _suboptimal = unsafe {
            self.swapchain_loader
                .queue_present(self.queue, &present_info)
        }?;

        Ok(())
    }

    pub fn profiler_scope(&self, cmd: &CommandBuffer, name: &str) -> () {}

    pub fn wait_idle(&self) -> anyhow::Result<()> {
        unsafe { self.device.device_wait_idle()? };
        Ok(())
    }
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
