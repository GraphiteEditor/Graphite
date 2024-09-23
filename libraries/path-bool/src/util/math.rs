use glam::{DVec2, FloatExt};
pub use std::f64::consts::PI;

pub fn lin_map(value: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
	((value - in_min) / (in_max - in_min)) * (out_max - out_min) + out_min
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
	a.lerp(b, t)
}

pub fn vector_angle(u: DVec2, v: DVec2) -> f64 {
	const EPS: f64 = 1e-12;

	let sign = u.x * v.y - u.y * v.x;

	if sign.abs() < EPS && (u + v).length_squared() < EPS * EPS {
		// TODO: `u` can be scaled
		return PI;
	}

	sign.signum() * (u.dot(v) / (u.length() * v.length())).acos()
}
