use std::ffi::CStr;

use ash::vk;

#[derive(Debug, Clone)]
pub struct PhysicalDeviceDesc {
    pub description: String,
    pub api_version: u32,
    pub vendor_id: u32,
    pub device_id: u32,
    pub device_local_memory: u64,
}

pub struct PhysicalDeviceIterator {
    entry: ash::Entry,
    instance: ash::Instance,
    pdevs: Vec<vk::PhysicalDevice>,
    index: u32,
}

impl PhysicalDeviceIterator {
    pub fn new() -> anyhow::Result<Self> {
        let entry = unsafe { ash::Entry::load() }?;

        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(
                    &vk::ApplicationInfo::default()
                        .engine_name(c"Deimos")
                        .engine_version(1)
                        .application_name(c"Deimos")
                        .application_version(1)
                        .api_version(vk::make_api_version(0, 1, 0, 0)),
                ),
                None,
            )
        }?;

        let pdevs = unsafe { instance.enumerate_physical_devices()? };

        Ok(Self {
            entry,
            instance,
            pdevs,
            index: 0,
        })
    }
}

impl Iterator for PhysicalDeviceIterator {
    type Item = (vk::PhysicalDevice, PhysicalDeviceDesc);

    fn next(&mut self) -> Option<Self::Item> {
        let pdev = *self.pdevs.get(self.index as usize)?;
        self.index += 1;
        let properties = unsafe { self.instance.get_physical_device_properties(pdev) };
        let memory = unsafe { self.instance.get_physical_device_memory_properties(pdev) };

        let device_local_memory = memory
            .memory_types
            .iter()
            .enumerate()
            .filter(|(_, mt)| {
                mt.property_flags
                    .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            })
            .map(|(idx, _)| memory.memory_heaps[memory.memory_types[idx].heap_index as usize].size)
            .sum();

        let desc = PhysicalDeviceDesc {
            description: unsafe {
                CStr::from_ptr(properties.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            },
            api_version: properties.api_version,
            vendor_id: properties.vendor_id,
            device_id: properties.device_id,
            device_local_memory,
        };

        Some((pdev, desc))
    }
}
