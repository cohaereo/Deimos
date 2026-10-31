use deimos_data::tfx::{TfxFeatureRenderer, features::dynamic::RenderStageSubscription};

use crate::features::FeatureRenderer;

slotmap::new_key_type! {
    pub struct RenderObjectHandle;
}

pub struct RenderObject {
    pub name: Box<str>,
    pub renderer: Box<dyn FeatureRenderer>,
    pub feature_type: TfxFeatureRenderer,
    pub stages: RenderStageSubscription,
}

impl RenderObject {
    pub fn new(
        name: impl AsRef<str>,
        kind: TfxFeatureRenderer,
        renderer: Box<dyn FeatureRenderer>,
    ) -> Self {
        Self {
            name: name.as_ref().into(),
            stages: renderer.subscribed_stages(),
            renderer,
            feature_type: kind,
        }
    }

    pub fn get_mut<T: FeatureRenderer>(&mut self) -> Option<&mut T> {
        self.renderer.as_any_mut().downcast_mut::<T>()
    }
}

unsafe impl Send for RenderObject {}
unsafe impl Sync for RenderObject {}
