use crate::Color;
use glam::{DAffine2, DVec2};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum RenderBoundingBox {
	#[default]
	None,
	Infinite,
	Rectangle([DVec2; 2]),
}

pub trait BoundingBox {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox;

	/// Returns the bounding box to use when sizing this value's thumbnail in the Layers panel.
	///
	/// Diverges from `bounding_box` for types where the rendering bounds wouldn't make a useful thumbnail frame.
	/// For instance, `GradientStops` is `Infinite` for rendering but returns the line's AABB here, so a `Table<Graphic>`
	/// group of a gradient and a vector frames around the vector's geometry rather than infinity.
	/// Types with no meaningful contribution (e.g., `Color`) return `Infinite` from both; the runtime substitutes a
	/// small fallback rectangle at the end if no finite bounds remain after combining.
	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox;
}

macro_rules! none_impl {
	($t:path) => {
		impl BoundingBox for $t {
			fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
				RenderBoundingBox::None
			}

			fn thumbnail_bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
				RenderBoundingBox::None
			}
		}
	};
}
none_impl!(bool);
none_impl!(f32);
none_impl!(f64);
none_impl!(DVec2);
none_impl!(String);

impl BoundingBox for Color {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		RenderBoundingBox::Infinite
	}

	fn thumbnail_bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		// A solid color has no intrinsic extent, so its container's other content frames the thumbnail
		RenderBoundingBox::Infinite
	}
}
