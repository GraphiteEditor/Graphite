pub mod cache;
pub use cache::BrushCache;

use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::render_complexity::RenderComplexity;
use core_types::{CacheHash, Color};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2, Vec2};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStyle {
	pub color: Color,
	pub diameter: f64,
	pub hardness: f64,
	pub flow: f64,
}

impl Default for BrushStyle {
	fn default() -> Self {
		Self {
			color: Color::BLACK,
			diameter: 20.,
			hardness: 0.8,
			flow: 1.,
		}
	}
}

#[derive(Clone, Debug, PartialEq, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Channel<T> {
	Uniform(T),
	Samples(Vec<T>),
}

impl<T: Copy> Channel<T> {
	pub fn get(&self, index: usize) -> T {
		match self {
			Self::Uniform(value) => *value,
			Self::Samples(values) => values[index],
		}
	}

	pub fn len(&self) -> Option<usize> {
		match self {
			Self::Uniform(_) => None,
			Self::Samples(values) => Some(values.len()),
		}
	}

	pub fn is_uniform(&self) -> bool {
		matches!(self, Self::Uniform(_))
	}

	pub fn is_empty(&self) -> bool {
		match self {
			Self::Uniform(_) => false,
			Self::Samples(values) => values.is_empty(),
		}
	}
}

unsafe impl<T: dyn_any::StaticTypeSized> dyn_any::StaticType for Channel<T> {
	type Static = Channel<T::Static>;
}

#[derive(Clone, Debug, PartialEq, CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stroke {
	pub position: Vec<DVec2>,
	pub pressure: Channel<f32>,
	pub tilt: Channel<Vec2>,
	pub twist: Channel<f32>,
	pub time: Channel<f64>,
	pub seed: u64,
}

impl Default for Stroke {
	fn default() -> Self {
		Self {
			position: Vec::new(),
			pressure: Channel::Uniform(1.),
			tilt: Channel::Uniform(Vec2::ZERO),
			twist: Channel::Uniform(0.),
			time: Channel::Uniform(0.),
			seed: 0,
		}
	}
}

impl Stroke {
	pub fn len(&self) -> usize {
		self.position.len()
	}

	pub fn is_empty(&self) -> bool {
		self.position.is_empty()
	}

	pub fn is_valid(&self) -> bool {
		let n = self.len();
		[self.pressure.len(), self.tilt.len(), self.twist.len(), self.time.len()].into_iter().flatten().all(|len| len == n)
	}

	pub fn sample(&self, index: usize) -> Sample {
		Sample {
			position: self.position[index],
			pressure: self.pressure.get(index),
			tilt: self.tilt.get(index),
			twist: self.twist.get(index),
			time: self.time.get(index),
		}
	}

	pub fn sample_lerp(&self, index: usize, t: f32) -> Sample {
		let a = self.sample(index);
		let b = self.sample(index + 1);
		let lerp = |a: f32, b: f32| a + (b - a) * t;
		Sample {
			position: a.position.lerp(b.position, t as f64),
			pressure: lerp(a.pressure, b.pressure),
			tilt: a.tilt.lerp(b.tilt, t),
			twist: lerp_angle(a.twist, b.twist, t),
			time: a.time + (b.time - a.time) * t as f64,
		}
	}

	pub fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
		(0..self.len()).map(|index| self.sample(index))
	}
}

impl BoundingBox for Stroke {
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		let Some(first) = self.position.first() else { return RenderBoundingBox::None };
		let (min, max) = self.position.iter().fold((*first, *first), |(min, max), &point| (min.min(point), max.max(point)));
		let corners = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)].map(|corner| transform.transform_point2(corner));
		let (min, max) = corners.iter().fold((corners[0], corners[0]), |(min, max), &point| (min.min(point), max.max(point)));
		RenderBoundingBox::Rectangle([min, max])
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox {
		self.bounding_box(transform, include_stroke)
	}
}

impl RenderComplexity for Stroke {
	fn render_complexity(&self) -> usize {
		self.len()
	}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
	pub position: DVec2,
	pub pressure: f32,
	pub tilt: Vec2,
	pub twist: f32,
	pub time: f64,
}

fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
	let delta = (b - a).rem_euclid(TAU);
	let delta = if delta > PI { delta - TAU } else { delta };
	a + delta * t
}
