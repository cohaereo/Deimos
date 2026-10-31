pub mod container;
pub mod definitions;
mod macros;
use std::{any::TypeId, fmt::Debug, sync::Arc};

use ash::vk;
pub use container::*;
pub use definitions::*;
use deimos_data::tfx::ExternIndex;
use glam::{Mat4, Vec4};

use crate::{
    asset::{Handle, texture::Texture},
    renderer::Renderer,
};

trait Extern {
    // fn get_field<T: Sized + 'static>(&self, offset: usize) -> Option<&T> {
    //     let (ptr, typeid) = self.get_field_ptr(offset)?;
    //     if TypeId::of::<T>() != typeid {
    //         return None;
    //     }

    //     let value = unsafe { &*(ptr as *const T) };
    //     Some(value)
    // }

    fn get_field_ptr(&self, offset: usize) -> Option<(*const (), TypeId)>;
    fn get_field_name(offset: usize) -> Option<&'static str>;
}

pub trait ExternAccessor {
    fn get_value_ptr(&self, index: ExternIndex, offset: usize) -> Option<(*const (), TypeId)>;

    fn get_global_channel(&self, index: u8) -> Vec4;
}

pub trait ExternAccessorExt {
    fn get_extern_value<U: Sized + Clone + ExternValue + 'static>(
        &self,
        index: ExternIndex,
        offset: usize,
    ) -> Option<U>;
}

impl ExternAccessorExt for &dyn ExternAccessor {
    fn get_extern_value<U: Sized + Clone + ExternValue + 'static>(
        &self,
        index: ExternIndex,
        offset: usize,
    ) -> Option<U> {
        let (ptr, typeid) = self.get_value_ptr(index, offset)?;
        if TypeId::of::<U>() != typeid {
            error!(
                "Extern type mismatch for {index:?}+0x{offset:X}: expected {:?} ({}), found {:?}",
                TypeId::of::<U>(),
                std::any::type_name::<U>(),
                typeid
            );
            return None;
        }
        let value = unsafe { &*ptr.cast::<U>() };
        Some(value.clone())
    }
}

impl<T: ExternAccessor> ExternAccessorExt for T {
    fn get_extern_value<U: Sized + Clone + ExternValue + 'static>(
        &self,
        index: ExternIndex,
        offset: usize,
    ) -> Option<U> {
        let (ptr, typeid) = self.get_value_ptr(index, offset)?;
        if TypeId::of::<U>() != typeid {
            error!(
                "Extern type mismatch for {index:?}+0x{offset:X}: expected {:?} ({}), found {:?}",
                TypeId::of::<U>(),
                std::any::type_name::<U>(),
                typeid
            );
            return None;
        }
        let value = unsafe { &*ptr.cast::<U>() };
        Some(value.clone())
    }
}

#[derive(Default, Clone)]
pub enum TextureView {
    #[default]
    None,
    View(vk::ImageView),
    Resource(Handle<Texture>),
}

impl TextureView {
    pub fn get_srv(&self) -> Option<vk::ImageView> {
        match self {
            Self::None => None,
            Self::View(resource_view) => Some(*resource_view),
            Self::Resource(handle) => handle.get_ref(|t| t.view),
        }
    }

    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Debug for TextureView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::View(view) => write!(f, "View({:?})", view),
            Self::Resource(texture) => write!(f, "Resource({})", texture.hash()),
            // TextureView::Raw(_) => write!(f, "Raw"),
        }
    }
}

impl From<vk::ImageView> for TextureView {
    fn from(value: vk::ImageView) -> Self {
        Self::View(value)
    }
}

impl From<Handle<Texture>> for TextureView {
    fn from(texture: Handle<Texture>) -> Self {
        Self::Resource(texture)
    }
}

#[derive(Default, Clone)]
pub enum Uav {
    #[default]
    None,
    View(vk::ImageView),
    // Surface(SurfaceHandle),
}

impl Uav {
    pub const fn get_uav(&self) -> Option<vk::ImageView> {
        match self {
            Self::None => None,
            Self::View(view) => Some(*view),
        }
    }
}

impl From<vk::ImageView> for Uav {
    fn from(value: vk::ImageView) -> Self {
        Self::View(value)
    }
}

impl Debug for Uav {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::View(view) => write!(f, "View({view:?})"),
            // Uav::Surface(surface) => write!(f, "Surface({surface:?})"),
        }
    }
}

// impl Uav {
//     pub fn get_uav<F>(&self, f: F)
//     where
//         F: FnOnce(&d3d11::UnorderedAccessView),
//     {
//         match self {
//             Uav::Surface(surface) => {
//                 if let Some(uav) = Renderer::instance().surfaces().get(*surface).uav.as_ref() {
//                     f(uav)
//                 }
//             }
//             Uav::None => {}
//         }
//     }
// }

// impl From<SurfaceHandle> for Uav {
//     fn from(surface: SurfaceHandle) -> Self {
//         Uav::Surface(surface)
//     }
// }

/// Marker trait for types that can be used as extern values in the TFX system.
pub trait ExternValue {}

impl ExternValue for f32 {}
impl ExternValue for Vec4 {}
impl ExternValue for Mat4 {}
impl ExternValue for TextureView {}
impl ExternValue for Uav {}

pub trait ExternDefault {
    fn extern_default() -> Self;
}

impl ExternDefault for TextureView {
    fn extern_default() -> Self {
        Self::None
    }
}

impl ExternDefault for Uav {
    fn extern_default() -> Self {
        Self::None
    }
}

impl ExternDefault for Vec4 {
    fn extern_default() -> Self {
        Self::ONE
    }
}

impl ExternDefault for Mat4 {
    fn extern_default() -> Self {
        Self::IDENTITY
    }
}

impl ExternDefault for f32 {
    fn extern_default() -> Self {
        1.0
    }
}

impl ExternDefault for u32 {
    fn extern_default() -> Self {
        0
    }
}

#[derive(Clone)]
pub enum BaseExternSource {
    None,
    Renderer(Arc<Renderer>),
}

impl ExternAccessor for BaseExternSource {
    fn get_value_ptr(&self, index: ExternIndex, offset: usize) -> Option<(*const (), TypeId)> {
        match self {
            Self::None => None,
            Self::Renderer(renderer) => renderer.externs.get_value_ptr(index, offset),
        }
    }

    fn get_global_channel(&self, index: u8) -> Vec4 {
        match self {
            Self::None => Vec4::ONE,
            Self::Renderer(renderer) => renderer.externs.get_global_channel(index),
        }
    }
}
