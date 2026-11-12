macro_rules! extern_container {
    (   pub struct ExternContainer {
            $($name:ident: $t:ident),*
        }
    ) => {
        /// Container holding all externs and global channels used in the renderer.
        pub struct ExternContainer {
            $(
            pub $name: Box<$t>,
            )*

            pub globals: [Vec4; 256],
            pub global_channel_frequency: [AtomicUsize; 256],
            pub global_ids: Vec<u32>,

            pub unk5e: [Vec4; 256],
            pub unk5e_frequency: [AtomicUsize; 256],
            pub unk5e_ids: Vec<u32>,

            generation: AtomicU64,
        }

        impl ExternContainer {
            // pub fn get_extern_value<T: Sized + Clone + 'static>(
            //     &self,
            //     index: ExternIndex,
            //     offset: usize,
            // ) -> Option<&T> {
            //     match index {
            //         $(
            //             ExternIndex::$t => self.$name.get_field(offset),
            //         )*
            //         _ => None,
            //     }
            // }

            pub fn get_extern_field_name(index: ExternIndex, offset: usize) -> Option<&'static str> {
                match index {
                    $(
                        ExternIndex::$t => $t::get_field_name(offset),
                    )*
                    _ => None,
                }
            }

            pub fn reset_global_channel_frequencies(&self) {
                for freq in self.global_channel_frequency.iter() {
                    freq.store(0, Ordering::Relaxed);
                }
                for freq in self.unk5e_frequency.iter() {
                    freq.store(0, Ordering::Relaxed);
                }
            }

            pub fn bump_generation(&self) {
                self.generation.fetch_add(1, Ordering::Relaxed);
            }

            pub fn generation_value(&self) -> u64 {
                self.generation.load(Ordering::Relaxed)
            }
        }

        impl ExternAccessor for ExternContainer {
            fn get_value_ptr(&self, index: ExternIndex, offset: usize) -> Option<(*const (), TypeId)> {
                match index {
                    $(
                        ExternIndex::$t => {
                            self.$name.get_field_ptr(offset)
                        }
                    )*
                    _ => None,
                }
            }

            fn get_global_channel(&self, index: u8) -> Vec4 {
                self.global_channel_frequency[index as usize].fetch_add(1, Ordering::Relaxed);
                self.globals[index as usize]
            }

            fn get_unk5e_channel(&self, index: u8) -> Vec4 {
                self.unk5e_frequency[index as usize].fetch_add(1, Ordering::Relaxed);
                self.unk5e[index as usize]
            }
        }

        impl Default for ExternContainer {
            fn default() -> Self {
                let global_channels = &crate::renderer::globals::RENDER_GLOBALS.global_channels;
                let mut globals = [Vec4::ONE; 256];
                globals[..global_channels.default_values.len()].copy_from_slice(&global_channels.default_values);

                let unk5e_channels = &crate::renderer::globals::RENDER_GLOBALS.unk5e_channels;
                let unk5e = [Vec4::ONE; 256];
                let mut unk5e_ids = vec![0; 256];

                for channel in &unk5e_channels.all_channels {
                    unk5e_ids[channel.index as usize] = channel.id;
                }

                let r = Self {
                    $(
                        $name: Default::default(),
                    )*
                    globals,
                    global_channel_frequency: [const { AtomicUsize::new(0) }; 256],
                    global_ids: global_channels.channel_ids.clone(),

                    unk5e,
                    unk5e_frequency: [const { AtomicUsize::new(0) }; 256],
                    unk5e_ids,

                    generation: AtomicU64::new(1),
                };

                // r.set_global_channel_by_id(743670137, Vec4::splat(0.1));

                r
            }
        }
    };
}

macro_rules! local_extern_container {
    ($($name:ident: $t:ident),*) => {
        /// Containers holding localized externs that allows for overriding externs for individual command lists
        #[derive(Clone)]
        pub struct LocalExternContainer {
            base: BaseExternSource,
            $(
            pub $name: Option<Box<$t>>,
            )*
        }

        impl LocalExternContainer {
            pub const fn new(base: BaseExternSource) -> Self {
                Self {
                    base,
                    $(
                        $name: None,
                    )*
                }
            }

            pub const fn base(&self) -> &BaseExternSource {
                &self.base
            }

            pub fn get_local_value_ptr(
                &self,
                index: ExternIndex,
                offset: usize,
            ) -> Option<(*const (), TypeId)> {
                match index {
                    $(
                        ExternIndex::$t => self.$name.as_ref().and_then(|e| e.get_field_ptr(offset)),
                    )*
                    _ => None,
                }
            }

            pub const fn has_local_override(&self, index: ExternIndex) -> bool {
                match index {
                    $(
                        ExternIndex::$t => self.$name.is_some(),
                    )*
                    _ => false,
                }
            }

            pub fn base_generation(&self) -> u64 {
                self.base.generation_value()
            }
        }

        impl ExternAccessor for LocalExternContainer {
            fn get_value_ptr(&self, index: ExternIndex, offset: usize) -> Option<(*const (), TypeId)> {
                match index {
                    $(
                        ExternIndex::$t => {
                            if let Some(e) = self.$name.as_ref() {
                                e.get_field_ptr(offset)
                            } else {
                                self.base.get_value_ptr(index, offset)
                            }
                        }
                    )*
                    _ => self.base.get_value_ptr(index, offset),
                }
            }

            fn get_global_channel(&self, index: u8) -> Vec4 {
                self.base.get_global_channel(index)
            }

            fn get_unk5e_channel(&self, index: u8) -> Vec4 {
                self.base.get_unk5e_channel(index)
            }
        }
    }
}

macro_rules! extern_struct {
    (struct $name:ident ($name_c:literal) {
        $(
            $(#[$field_attr:meta])*
            $field_offset:literal => $field:ident: $field_type:ty $(> default($default_value:expr))?,
        )*
    }) => {
        #[repr(C)]
        #[derive(Clone, Debug)]
        pub struct $name {
            $(
                $(#[$field_attr])*
                pub $field: $field_type,
            )*
        }

        impl Extern for $name {
            fn get_field_ptr(&self, offset: usize) -> Option<(*const (), TypeId)> {
                let ptr = (self as *const Self).cast::<u8>();

                match offset {
                    $($field_offset => {
                        unsafe {
                            let ptr = ptr.add(std::mem::offset_of!(Self, $field));
                            Some((ptr.cast::<()>(), std::any::TypeId::of::<$field_type>()))
                        }
                    })*
                    _ => None
                }
            }

            fn get_field_name(offset: usize) -> Option<&'static str> {
                match offset {
                    $($field_offset => Some(stringify!($field)),)*
                    _ => None
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $(if true { $default_value } else )* {
                        ExternDefault::extern_default()
                    },)*
                }
            }
        }
    };
}

pub(super) use extern_container;
pub(super) use extern_struct;
pub(super) use local_extern_container;
