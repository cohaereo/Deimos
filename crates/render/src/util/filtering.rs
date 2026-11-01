use glam::Vec3;
use std::f32::consts::PI;

pub const NUM_SAMPLES: u32 = 1024;

pub fn radical_inverse_vdc(mut bits: u32) -> f32 {
    bits = bits.rotate_right(16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xaaaa_aaaa) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xcccc_cccc) >> 2);
    bits = ((bits & 0x0f0f_0f0f) << 4) | ((bits & 0xf0f0_f0f0) >> 4);
    bits = ((bits & 0x00ff_00ff) << 8) | ((bits & 0xff00_ff00) >> 8);
    bits as f32 * 2.328_306_4e-10
}

pub fn hammersley(i: u32, n: u32) -> (f32, f32) {
    (i as f32 / n as f32, radical_inverse_vdc(i))
}

pub fn uv_to_dir(u: f32, v: f32) -> Vec3 {
    let phi = (u - 0.5) * 2.0 * PI;
    let theta = (v - 0.5) * PI;
    let cos_theta = theta.cos();
    Vec3::new(cos_theta * phi.cos(), theta.sin(), cos_theta * phi.sin())
}

pub fn dir_to_uv(dir: Vec3) -> (f32, f32) {
    let phi = dir.z.atan2(dir.x);
    let theta = dir.y.clamp(-1.0, 1.0).asin();
    (phi / (2.0 * PI) + 0.5, theta / PI + 0.5)
}

pub fn importance_sample_ggx(xi: (f32, f32), alpha: f32, n: Vec3) -> Vec3 {
    let (xi_x, xi_y) = xi;
    let a2 = alpha * alpha;
    let phi = 2.0 * PI * xi_x;
    let cos_theta = ((1.0 - xi_y) / (a2 - 1.0).mul_add(xi_y, 1.0)).sqrt();
    let sin_theta = f32::mul_add(cos_theta, -cos_theta, 1.0).max(0.0).sqrt();

    let h_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);

    let up = if n.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent = up.cross(n).normalize();
    let bitangent = n.cross(tangent);

    (tangent * h_local.x + bitangent * h_local.y + n * h_local.z).normalize()
}

pub fn sample_base_bilinear(base: &[f32], width: usize, height: usize, dir: Vec3) -> [f32; 3] {
    let (u, v) = dir_to_uv(dir);
    let u = u.rem_euclid(1.0);
    let v = v.clamp(0.0, 1.0);

    let px = f32::mul_add(u, width as f32, -0.5);
    let py = f32::mul_add(v, height as f32, -0.5);

    let x0 = (px.floor() as isize).rem_euclid(width as isize) as usize;
    let y0 = (py.floor() as usize).min(height - 1);
    let x1 = (x0 + 1) % width;
    let y1 = (y0 + 1).min(height - 1);
    let fx = px.fract().max(0.0);
    let fy = py.fract().max(0.0);

    let fetch = |x: usize, y: usize| -> [f32; 3] {
        let i = (y * width + x) * 4;
        [base[i], base[i + 1], base[i + 2]]
    };

    let blerp = |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| -> [f32; 3] {
        let lerp = |p: f32, q: f32, t: f32| (q - p).mul_add(t, p);
        [
            lerp(lerp(a[0], b[0], fx), lerp(c[0], d[0], fx), fy),
            lerp(lerp(a[1], b[1], fx), lerp(c[1], d[1], fx), fy),
            lerp(lerp(a[2], b[2], fx), lerp(c[2], d[2], fx), fy),
        ]
    };

    blerp(fetch(x0, y0), fetch(x1, y0), fetch(x0, y1), fetch(x1, y1))
}

pub fn d_ggx(n_dot_h: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let denom = (n_dot_h * n_dot_h).mul_add(a2 - 1.0, 1.0);
    a2 / (PI * denom * denom)
}

pub fn sample_mip_bilinear(
    mip_chain: &[Vec<f32>],
    mip_widths: &[usize],
    mip_heights: &[usize],
    dir: Vec3,
    mip_level: f32,
) -> [f32; 3] {
    let mip_lo = (mip_level.floor() as usize).min(mip_chain.len() - 1);
    let mip_hi = (mip_lo + 1).min(mip_chain.len() - 1);
    let frac = mip_level.fract().max(0.0);

    let s0 = sample_base_bilinear(
        &mip_chain[mip_lo],
        mip_widths[mip_lo],
        mip_heights[mip_lo],
        dir,
    );
    let s1 = sample_base_bilinear(
        &mip_chain[mip_hi],
        mip_widths[mip_hi],
        mip_heights[mip_hi],
        dir,
    );

    [
        (s1[0] - s0[0]).mul_add(frac, s0[0]),
        (s1[1] - s0[1]).mul_add(frac, s0[1]),
        (s1[2] - s0[2]).mul_add(frac, s0[2]),
    ]
}
