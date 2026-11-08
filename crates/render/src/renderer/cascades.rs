use inline_tweak::tweak;

pub struct CascadeCalculator;

impl CascadeCalculator {
    pub const MAX_CASCADES: usize = 4;
    pub const MAX_FAR_PLANE: f32 = 1000.0;
    pub const SHADOW_MAP_RESOLUTION: u32 = 4096;

    pub fn get_depth_range(cascade_index: usize, far_plane: f32) -> Option<(f32, f32)> {
        let max_far_plane = tweak!(1000.0);
        let c0 = tweak!(80.0);
        let c1 = tweak!(30.0);
        let c2 = tweak!(5.0);

        let far = far_plane.min(max_far_plane);
        Some(match cascade_index {
            0 => (0.0, far / c0),
            1 => (far / c0, far / c1),
            2 => (far / c1, far / c2),
            3 => (far / c2, far),
            _ => return None,
        })
    }
}
