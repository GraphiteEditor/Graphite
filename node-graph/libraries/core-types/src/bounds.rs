use crate::Color;
use glam::{DAffine2, DVec2};

/// Fallback rectangle used as the thumbnail bounding box for types whose normal bounding box is
/// `RenderBoundingBox::Infinite` (currently just solid `Color`). Thumbnail rendering needs a finite
/// preview area, so this is what callers substitute when their thumbnail bounding box query returns
/// `Infinite`, either by returning it directly from `thumbnail_bounding_box` or by mapping it from
/// `Infinite` at the call site.
pub const DEFAULT_THUMBNAIL_BOUNDS: [DVec2; 2] = [DVec2::ZERO, DVec2::new(300., 200.)];

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum RenderBoundingBox {
	#[default]
	None,
	Infinite,
	Rectangle([DVec2; 2]),
}

pub trait BoundingBox {
	fn bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox;

	/// Returns a finite bounding box suitable for rendering a thumbnail.
	///
	/// Differs from `bounding_box` only for types that would otherwise return
	/// `RenderBoundingBox::Infinite` (e.g., `Color`, `GradientStops`).
	/// Those substitute a finite fallback rectangle so the thumbnail has a defined area to render into.
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
		// A solid color has no intrinsic extent (its `bounding_box` is `Infinite`),
		// so we substitute a finite fallback so the thumbnail has a defined area to fill.
		RenderBoundingBox::Rectangle(DEFAULT_THUMBNAIL_BOUNDS)
	}
}
