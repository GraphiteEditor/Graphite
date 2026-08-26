use crate::Color;
use crate::lane::{LaneColumn, LaneSource};
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
	/// For instance, `GradientStops` is `Infinite` for rendering but returns the line's AABB here, so a `List<Graphic>`
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

/// Combined bounding box of a lane source's elements, composing each lane's
/// transform attribute with the given transform.
pub fn lane_bounding_box<S: LaneSource>(source: &S, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox
where
	S::Element: BoundingBox,
{
	let mut combined_bounds = None;

	let transforms = source.column::<crate::attribute::Transform>();
	for lane in 0..source.lane_count() {
		let Some(element) = source.element(lane) else { continue };
		let lane_transform: DAffine2 = transforms.get(lane);
		match element.bounding_box(transform * lane_transform, include_stroke) {
			RenderBoundingBox::None => continue,
			RenderBoundingBox::Infinite => return RenderBoundingBox::Infinite,
			RenderBoundingBox::Rectangle(bounds) => match combined_bounds {
				Some(existing) => combined_bounds = Some(crate::math::quad::Quad::combine_bounds(existing, bounds)),
				None => combined_bounds = Some(bounds),
			},
		}
	}

	match combined_bounds {
		Some(bounds) => RenderBoundingBox::Rectangle(bounds),
		None => RenderBoundingBox::None,
	}
}

/// As [`lane_bounding_box`], but `Infinite` lanes are skipped (rather than
/// propagating outward) so a finite sibling in a mixed group dictates the
/// framing.
pub fn lane_thumbnail_bounding_box<S: LaneSource>(source: &S, transform: DAffine2, include_stroke: bool) -> RenderBoundingBox
where
	S::Element: BoundingBox,
{
	let mut combined_bounds = None;
	let mut any_infinite = false;

	let transforms = source.column::<crate::attribute::Transform>();
	for lane in 0..source.lane_count() {
		let Some(element) = source.element(lane) else { continue };
		let lane_transform: DAffine2 = transforms.get(lane);
		match element.thumbnail_bounding_box(transform * lane_transform, include_stroke) {
			RenderBoundingBox::None => continue,
			RenderBoundingBox::Infinite => any_infinite = true,
			RenderBoundingBox::Rectangle(bounds) => match combined_bounds {
				Some(existing) => combined_bounds = Some(crate::math::quad::Quad::combine_bounds(existing, bounds)),
				None => combined_bounds = Some(bounds),
			},
		}
	}

	match (combined_bounds, any_infinite) {
		(Some(bounds), _) => RenderBoundingBox::Rectangle(bounds),
		(None, true) => RenderBoundingBox::Infinite,
		(None, false) => RenderBoundingBox::None,
	}
}
