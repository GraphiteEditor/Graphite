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

// TODO: Eventually remove this migration document upgrade code
pub fn migrate_color<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<crate::table::Table<graphene_core_shaders::color::Color>, D::Error> {
	use crate::table::Table;
	use graphene_core_shaders::color::Color;
	use serde::Deserialize;

	#[derive(serde::Serialize, serde::Deserialize)]
	#[serde(untagged)]
	enum ColorFormat {
		Color(Color),
		OptionalColor(Option<Color>),
		ColorTable(Table<Color>),
	}

	Ok(match ColorFormat::deserialize(deserializer)? {
		ColorFormat::Color(color) => Table::new_from_element(color),
		ColorFormat::OptionalColor(color) => {
			if let Some(color) = color {
				Table::new_from_element(color)
			} else {
				Table::new()
			}
		}
		ColorFormat::ColorTable(color_table) => color_table,
	})
}
