use std::sync::Arc;

use ash::vk;

use crate::gpu::{Gpu, command_list::ConstantBufferView};

#[derive(Clone, Copy, Debug, Default)]
pub struct DescriptorSizes {
    pub sampler: usize,
    pub sampled_image: usize,
    pub storage_image: usize,
    pub uniform_texel_buffer: usize,
    pub storage_texel_buffer: usize,
    pub uniform_buffer: usize,
    pub storage_buffer: usize,
}

pub struct DescriptorWriter<'a> {
    gpu: &'a Arc<Gpu>,
    offsets: &'a [u64; 32],
    base_ptr: *mut u8,
}

impl<'a> DescriptorWriter<'a> {
    pub const fn new(gpu: &'a Arc<Gpu>, offsets: &'a [u64; 32], base_ptr: *mut u8) -> Self {
        Self {
            gpu,
            offsets,
            base_ptr,
        }
    }

    #[allow(clippy::mut_from_ref)]
    fn descriptor_ptr(&self, binding: u32, size: usize) -> &mut [u8] {
        unsafe {
            let offset = self.offsets[binding as usize];

            debug_assert!(size > 0, "descriptor size is zero");
            std::slice::from_raw_parts_mut(self.base_ptr.add(offset as usize), size)
        }
    }

    fn write(&self, binding: u32, info: &vk::DescriptorGetInfoEXT<'_>, size: usize) {
        if size == 0 {
            error!(
                "Descriptor size for {:?} is zero, descriptor buffer is not supported?",
                info.ty
            );
            return;
        }

        let data = self.descriptor_ptr(binding, size);
        unsafe {
            self.gpu.descriptor_buffer.get_descriptor(info, data);
        }
    }

    pub fn write_uniform_buffer(&self, binding: u32, cbv: Option<ConstantBufferView>) {
        let cbv = cbv.unwrap_or_else(|| ConstantBufferView {
            addr: self.gpu.frame().upload.null(),
            size: 16,
        });

        let address_info = vk::DescriptorAddressInfoEXT::default()
            .address(cbv.addr)
            .range(cbv.size)
            .format(vk::Format::UNDEFINED);
        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .data(vk::DescriptorDataEXT {
                p_uniform_buffer: &raw const address_info,
            });

        self.write(binding, &get_info, self.gpu.descriptor_sizes.uniform_buffer);
    }

    pub fn write_storage_buffer(&self, binding: u32, cbv: Option<ConstantBufferView>) {
        let cbv = cbv.unwrap_or_else(|| ConstantBufferView {
            addr: self.gpu.frame().upload.null(),
            size: 16,
        });

        let address_info = vk::DescriptorAddressInfoEXT::default()
            .address(cbv.addr)
            .range(cbv.size)
            .format(vk::Format::UNDEFINED);
        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::STORAGE_BUFFER)
            .data(vk::DescriptorDataEXT {
                p_storage_buffer: &raw const address_info,
            });

        self.write(binding, &get_info, self.gpu.descriptor_sizes.storage_buffer);
    }

    pub fn write_uniform_texel_buffer(
        &self,
        binding: u32,
        buffer: Option<crate::gpu::command_list::BufferResource>,
    ) {
        let (addr, size, format) = buffer.map_or_else(
            || (self.gpu.frame().upload.null(), 16, vk::Format::R32_UINT),
            |b| (b.addr, b.size, b.format),
        );

        let address_info = vk::DescriptorAddressInfoEXT::default()
            .address(addr)
            .range(size)
            .format(format);
        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::UNIFORM_TEXEL_BUFFER)
            .data(vk::DescriptorDataEXT {
                p_uniform_texel_buffer: &raw const address_info,
            });

        self.write(
            binding,
            &get_info,
            self.gpu.descriptor_sizes.uniform_texel_buffer,
        );
    }

    pub fn write_storage_texel_buffer(
        &self,
        binding: u32,
        buffer: Option<crate::gpu::command_list::BufferResource>,
    ) {
        let (addr, size, format) = buffer.map_or_else(
            || (self.gpu.frame().upload.null(), 16, vk::Format::R32_UINT),
            |b| (b.addr, b.size, b.format),
        );

        let address_info = vk::DescriptorAddressInfoEXT::default()
            .address(addr)
            .range(size)
            .format(format);
        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::STORAGE_TEXEL_BUFFER)
            .data(vk::DescriptorDataEXT {
                p_storage_texel_buffer: &raw const address_info,
            });

        self.write(
            binding,
            &get_info,
            self.gpu.descriptor_sizes.storage_texel_buffer,
        );
    }

    pub fn write_sampler(&self, binding: u32, sampler: Option<vk::Sampler>) {
        let sampler = sampler.unwrap_or(self.gpu.default_sampler);
        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::SAMPLER)
            .data(vk::DescriptorDataEXT {
                p_sampler: &raw const sampler,
            });

        self.write(binding, &get_info, self.gpu.descriptor_sizes.sampler);
    }

    pub fn write_sampled_image(&self, binding: u32, view: Option<vk::ImageView>) {
        let view = view.unwrap_or_else(|| self.fallback_texture_view());

        let image_info = vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            .sampler(vk::Sampler::null());

        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::SAMPLED_IMAGE)
            .data(vk::DescriptorDataEXT {
                p_sampled_image: &raw const image_info,
            });

        self.write(binding, &get_info, self.gpu.descriptor_sizes.sampled_image);
    }

    pub fn write_storage_image(
        &self,
        binding: u32,
        view: Option<vk::ImageView>,
        image_layout: vk::ImageLayout,
    ) {
        let view = view.unwrap_or_else(|| self.gpu.null_storage_image_view());
        let image_info = vk::DescriptorImageInfo::default()
            .image_view(view)
            .image_layout(image_layout)
            .sampler(vk::Sampler::null());

        let get_info = vk::DescriptorGetInfoEXT::default()
            .ty(vk::DescriptorType::STORAGE_IMAGE)
            .data(vk::DescriptorDataEXT {
                p_storage_image: &raw const image_info,
            });

        self.write(binding, &get_info, self.gpu.descriptor_sizes.storage_image);
    }

    fn fallback_texture_view(&self) -> vk::ImageView {
        self.gpu
            .magic_textures
            .get_texture(
                self.gpu,
                "descriptor-fallback".to_string(),
                Some([0, 0, 0, 0]),
            )
            .unwrap_or_else(|e| {
                error!("Failed to create fallback texture for descriptor: {e:?}");
                vk::ImageView::null()
            })
    }
}
