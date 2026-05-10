// TODO(TrueDoctor): Replace this with the more idiomatic approach instead of using `trait Clampable`.

/// A trait for types that can be clamped within a min/max range defined by f64.
pub trait Clampable: Sized {
	/// Clamps the value to be no less than `min`.
	fn clamp_hard_min(self, min: f64) -> Self;
	/// Clamps the value to be no more than `max`.
	fn clamp_hard_max(self, max: f64) -> Self;
}

// Implement for common numeric types
macro_rules! impl_clampable_float {
	($($ty:ty),*) => {
		$(
			impl Clampable for $ty {
				#[inline(always)]
				fn clamp_hard_min(self, min: f64) -> Self {
					self.max(min as $ty)
				}
				#[inline(always)]
				fn clamp_hard_max(self, max: f64) -> Self {
					self.min(max as $ty)
				}
			}
		)*
	};
}
impl_clampable_float!(f32, f64);

macro_rules! impl_clampable_int {
	($($ty:ty),*) => {
		$(
			impl Clampable for $ty {
				#[inline(always)]
				fn clamp_hard_min(self, min: f64) -> Self {
					// Using try_from to handle potential range issues safely, though min should ideally be valid.
					// Consider using a different approach if f64 precision vs integer range is a concern.
					<$ty>::try_from(min.ceil() as i64).ok().map_or(self, |min_val| self.max(min_val))
				}
				#[inline(always)]
				fn clamp_hard_max(self, max: f64) -> Self {
					<$ty>::try_from(max.floor() as i64).ok().map_or(self, |max_val| self.min(max_val))
				}
			}
		)*
	};
}
// Add relevant integer types (adjust as needed)
impl_clampable_int!(u32, u64, i32, i64);

// Implement for DVec2 (component-wise clamping)
use glam::DVec2;
impl Clampable for DVec2 {
	#[inline(always)]
	fn clamp_hard_min(self, min: f64) -> Self {
		self.max(DVec2::splat(min))
	}
	#[inline(always)]
	fn clamp_hard_max(self, max: f64) -> Self {
		self.min(DVec2::splat(max))
	}
}

// Forward Clampable through `Item<T>` so the macro-generated clamp can run on a wrapped value.
impl<T: Clampable> Clampable for crate::list::Item<T> {
	#[inline(always)]
	fn clamp_hard_min(self, min: f64) -> Self {
		let (element, attributes) = self.into_parts();
		Self::from_parts(element.clamp_hard_min(min), attributes)
	}
	#[inline(always)]
	fn clamp_hard_max(self, max: f64) -> Self {
		let (element, attributes) = self.into_parts();
		Self::from_parts(element.clamp_hard_max(max), attributes)
	}
}

#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
struct LegacyTable<T> {
	#[serde(alias = "instances", alias = "instance")]
	element: Vec<T>,
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_to_optional_color<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<no_std_types::color::Color>, D::Error> {
	use no_std_types::color::Color;
	use serde::Deserialize;

	#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum ColorFormat {
		OptionalColor(Option<Color>),
		List(LegacyTable<Color>),
	}

	Ok(match ColorFormat::deserialize(deserializer)? {
		ColorFormat::OptionalColor(color) => color,
		ColorFormat::List(list) => list.element.into_iter().next(),
	})
}

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_to_f64_array<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<f64>, D::Error> {
	use serde::Deserialize;

	#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
	#[cfg_attr(feature = "serde", serde(untagged))]
	enum F64ArrayFormat {
		Array(Vec<f64>),
		List(LegacyTable<f64>),
	}

	Ok(match F64ArrayFormat::deserialize(deserializer)? {
		F64ArrayFormat::Array(values) => values,
		F64ArrayFormat::List(list) => list.element,
	})
}
