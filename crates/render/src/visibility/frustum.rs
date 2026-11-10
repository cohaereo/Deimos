use std::ops::Deref;

use deimos_data::tfx::geometry::{AxisAlignedBBox, SphereBounds};
use glam::{Mat4, Vec3, Vec3A, Vec4, Vec4Swizzles, vec4};

use crate::camera::Camera;

#[derive(Default, Clone)]
pub struct Plane(pub Vec4);

impl Plane {
    pub fn normal(&self) -> Vec3A {
        self.xyz().into()
    }

    pub fn intersect(p0: &Self, p1: &Self, p2: &Self) -> Vec3A {
        let bxc = p1.normal().cross(p2.normal());
        let cxa = p2.normal().cross(p0.normal());
        let axb = p0.normal().cross(p1.normal());
        let r = -bxc * p0.w - cxa * p1.w - axb * p2.w;
        r * (1.0 / bxc.dot(p0.normal()))
    }

    pub fn normalized(&self) -> Self {
        Self(self.0 / self.xyz().length())
    }

    pub fn get_signed_distance(&self, point: Vec3A) -> f32 {
        self.normal().dot(point) + self.w
    }
}

impl Deref for Plane {
    type Target = Vec4;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Default, Clone)]
pub struct Frustum {
    pub planes: [Plane; 6],
    pub points: [Vec3A; 8],
    plane_abs_normals: [Vec3A; 6],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AabbVisibility {
    Outside,
    Intersecting,
    Inside,
}

impl Frustum {
    const LEFT: usize = 0;
    const RIGHT: usize = 1;
    const BOTTOM: usize = 2;
    const TOP: usize = 3;
    const NEAR: usize = 4;
    const FAR: usize = 5;

    #[rustfmt::skip]
    pub fn new(planes: [Plane; 6]) -> Self {
        let points = [
            Plane::intersect(&planes[Self::NEAR], &planes[Self::LEFT], &planes[Self::BOTTOM]),
            Plane::intersect(&planes[Self::NEAR], &planes[Self::LEFT], &planes[Self::TOP]),
            Plane::intersect(&planes[Self::NEAR], &planes[Self::RIGHT], &planes[Self::TOP]),
            Plane::intersect(&planes[Self::NEAR], &planes[Self::RIGHT], &planes[Self::BOTTOM]),
            Plane::intersect(&planes[Self::FAR], &planes[Self::LEFT], &planes[Self::BOTTOM]),
            Plane::intersect(&planes[Self::FAR], &planes[Self::LEFT], &planes[Self::TOP]),
            Plane::intersect(&planes[Self::FAR], &planes[Self::RIGHT], &planes[Self::TOP]),
            Plane::intersect(&planes[Self::FAR], &planes[Self::RIGHT], &planes[Self::BOTTOM]),
        ];

        let plane_abs_normals = std::array::from_fn(|i| planes[i].normal().abs());

        Self {
            planes,
            points,
            plane_abs_normals,
        }
    }

    pub fn from_camera(cam: &Camera) -> Self {
        Self::from_view_and_projection(cam.view_matrix(), cam.projection_matrix_standard())
    }

    pub fn from_view_and_projection(view: Mat4, projection: Mat4) -> Self {
        Self::from_world_to_projective(projection * view)
    }

    pub fn from_world_to_projective(world_to_projective: Mat4) -> Self {
        let cols = world_to_projective.to_cols_array_2d();

        let left = Plane(vec4(
            cols[0][3] + cols[0][0],
            cols[1][3] + cols[1][0],
            cols[2][3] + cols[2][0],
            cols[3][3] + cols[3][0],
        ));

        let right = Plane(vec4(
            cols[0][3] - cols[0][0],
            cols[1][3] - cols[1][0],
            cols[2][3] - cols[2][0],
            cols[3][3] - cols[3][0],
        ));

        let top = Plane(vec4(
            cols[0][3] - cols[0][1],
            cols[1][3] - cols[1][1],
            cols[2][3] - cols[2][1],
            cols[3][3] - cols[3][1],
        ));

        let bottom = Plane(vec4(
            cols[0][3] + cols[0][1],
            cols[1][3] + cols[1][1],
            cols[2][3] + cols[2][1],
            cols[3][3] + cols[3][1],
        ));

        let near = Plane(vec4(cols[0][2], cols[1][2], cols[2][2], cols[3][2]));

        let far = Plane(vec4(
            cols[0][3] - cols[0][2],
            cols[1][3] - cols[1][2],
            cols[2][3] - cols[2][2],
            cols[3][3] - cols[3][2],
        ));

        Self::new([left, right, bottom, top, near, far])
    }

    pub fn aabb_intersecting(&self, bb: &AxisAlignedBBox) -> bool {
        self.aabb_test(bb) != AabbVisibility::Outside
    }

    pub fn aabb_test(&self, bb: &AxisAlignedBBox) -> AabbVisibility {
        let extents = Vec3A::from(bb.extents()) * 0.5;
        let center = Vec3A::from(bb.centroid()).extend(1.0);
        let mut inside = true;

        for i in 0..6 {
            let distance = center.dot(self.planes[i].0);
            let radius = extents.dot(self.plane_abs_normals[i]);
            if distance < -radius {
                return AabbVisibility::Outside;
            }
            inside &= distance >= radius;
        }

        if inside {
            AabbVisibility::Inside
        } else {
            AabbVisibility::Intersecting
        }
    }

    pub fn sphere_intersecting(&self, sphere: &SphereBounds) -> bool {
        for plane in &self.planes {
            if plane.get_signed_distance(sphere.center.into()) < -sphere.radius {
                return false;
            }
        }
        true
    }

    pub fn center(&self) -> Vec3 {
        Vec3::from(self.points.iter().sum::<Vec3A>()) / self.points.len() as f32
    }
}
