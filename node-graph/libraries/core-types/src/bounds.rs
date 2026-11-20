use crate::Color;
use crate::list::{Item, List};
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
	/// For instance, `Gradient` is `Infinite` for rendering but returns the line's AABB here, so a `List<Graphic>`
	/// group of a gradient and a vector frames around the vector's geometry rather than infinity.
	/// Types with no meaningful contribution (e.g., `Color`) return `Infinite` from both; the runtime substitutes a
	/// small fallback rectangle at the end if no finite bounds remain after combining.
	fn thumbnail_bounding_box(&self, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox;

	/// The bounds of an item's element, for the thumbnail when `thumbnail` is set. A type with geometry that
	/// partly rides its item's attributes overrides this to read them, and the rest defer to the element alone.
	fn bounding_box_in_item(item: &Item<Self>, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> RenderBoundingBox
	where
		Self: Sized,
	{
		let element = item.element();
		if thumbnail {
			element.thumbnail_bounding_box(transform, include_stroke)
		} else {
			element.bounding_box(transform, include_stroke)
		}
	}

	/// As [`Self::bounding_box_in_item`], for the element at one index of a list.
	fn bounding_box_in_list(list: &List<Self>, index: usize, transform: DAffine2, include_stroke: bool, thumbnail: bool) -> RenderBoundingBox
	where
		Self: Sized,
	{
		let Some(element) = list.element(index) else { return RenderBoundingBox::None };
		if thumbnail {
			element.thumbnail_bounding_box(transform, include_stroke)
		} else {
			element.bounding_box(transform, include_stroke)
		}
	}
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
