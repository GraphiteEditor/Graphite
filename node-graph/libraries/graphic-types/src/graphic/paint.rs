//! The paint column level: fill and stroke read as lane columns and threaded down to the elements they reach.

use super::{Graphic, IntoGraphicList};
use crate::markers::{ATTR_FILL, ATTR_STROKE, Fill, Stroke};
use core_types::ATTR_TRANSFORM;
use core_types::attribute::{Attribute, Opacity};
use core_types::lane::{LaneColumn, LaneSource};
use core_types::list::{ItemAttributeValues, List};
use glam::DAffine2;
use vector_types::Vector;

/// Whether a normalized paint graphic list actually carries renderable paint.
/// A 0-item list, or a list whose first graphic is empty, is treated as no paint.
pub fn is_paint_present(graphic_list: &List<Graphic>) -> bool {
	graphic_list.element(0).is_some_and(|graphic| !graphic.is_empty())
}

/// Look up the paint graphics stored under the marker `A`, in the canonical `List<Graphic>` form.
pub fn paint_graphics<'a, A, S>(source: &'a S, index: usize) -> Option<&'a List<Graphic<'static>>>
where
	S: LaneSource,
	A: Attribute<Value<'a> = Option<&'a List<Graphic<'static>>>>,
{
	source
		.attr::<A>(index)
		// Treat a blank paint attribute as absent so an empty attribute doesn't count as painted
		.filter(|graphic_list| is_paint_present(graphic_list))
}

/// Whether the item carries a non-blank canonical `List<Graphic>` paint under the marker `A`,
/// checked by borrowing without cloning the renderable list.
pub fn has_paint<'a, A, S>(source: &'a S, index: usize) -> bool
where
	S: LaneSource,
	A: Attribute<Value<'a> = Option<&'a List<Graphic<'static>>>>,
{
	paint_graphics::<A, S>(source, index).is_some()
}

/// Whether every lane of a vector source draws as a plain clip path: fully
/// opaque, fill absent or opaque, stroke invisible or fully transparent.
pub fn vector_can_reduce_to_clip_path<S: LaneSource<Element = Vector>>(source: &S) -> bool {
	(0..source.lane_count()).all(|index| {
		let Some(element) = source.element(index) else { return false };
		let opacity: f64 = source.attr::<Opacity>(index);

		let fill_opaque_or_absent = paint_graphics::<Fill, _>(source, index).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_opaque()));

		let stroke_invisible_or_transparent = element.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke())
			|| paint_graphics::<Stroke, _>(source, index).is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_fully_transparent()));

		opacity > 1. - f64::EPSILON && fill_opaque_or_absent && stroke_invisible_or_transparent
	})
}

/// The paint a lane carries for its interiors, in the reference form
/// [`PaintOverlay`] threads down.
#[derive(Clone, Copy, Default)]
pub struct LanePaint<'a> {
	pub fill: Option<&'a List<Graphic<'static>>>,
	pub stroke: Option<&'a List<Graphic<'static>>>,
}

impl<'a> LanePaint<'a> {
	pub const NONE: Self = Self { fill: None, stroke: None };

	pub fn is_present(&self) -> bool {
		self.fill.is_some() || self.stroke.is_some()
	}
}

/// A source's fill and stroke columns, resolved once for per-lane reads.
pub struct PaintColumns<'a, S: LaneSource + 'a> {
	fill: S::Column<'a, Fill>,
	stroke: S::Column<'a, Stroke>,
}

impl<'a, S: LaneSource> PaintColumns<'a, S> {
	pub fn new(source: &'a S) -> Self {
		Self {
			fill: source.column::<Fill>(),
			stroke: source.column::<Stroke>(),
		}
	}

	/// The lane's present, non-blank paint.
	pub fn read(&self, lane: usize) -> LanePaint<'a> {
		let present = |value: Option<Option<&'a List<Graphic<'static>>>>| value.flatten().filter(|list| is_paint_present(list));
		LanePaint {
			fill: present(self.fill.try_get(lane)),
			stroke: present(self.stroke.try_get(lane)),
		}
	}
}

/// How far a lane's paint reaches into the element beneath it, mirroring the
/// legacy conversion's paint push: vector interiors directly and vector
/// children of a nested graphic list, one level deep.
#[derive(Clone, Copy)]
pub struct PaintReach<'a> {
	pub paint: LanePaint<'a>,
	hops: u8,
}

impl<'a> PaintReach<'a> {
	pub const NONE: Self = Self { paint: LanePaint::NONE, hops: 0 };

	/// The lane's effective reach: an inherited paint stays authoritative
	/// (lane paint below a push's origin is inert in the legacy model), an
	/// absent one reads the lane's own paint.
	pub fn for_lane<S: LaneSource>(self, columns: &PaintColumns<'a, S>, index: usize) -> Self {
		match self.paint.is_present() {
			true => self,
			false => Self { paint: columns.read(index), hops: 2 },
		}
	}

	pub fn applies(&self) -> bool {
		self.hops > 0 && self.paint.is_present()
	}

	/// The reach one graphic nesting level further down.
	pub fn nested(self) -> Self {
		Self {
			paint: self.paint,
			hops: self.hops.saturating_sub(1),
		}
	}

	/// The reach entering a group's own graphic run: a spent or absent reach
	/// resets so the group's own lane paint applies at its own boundary.
	pub fn into_group_graphics(self) -> Self {
		match self.applies() {
			true => self.nested(),
			false => Self::NONE,
		}
	}
}

/// A source with a lane's paint forced over its fill and stroke columns,
/// reaching the interiors the legacy conversion's paint push reached.
pub struct PaintOverlay<'a, S> {
	inner: &'a S,
	paint: LanePaint<'a>,
}

impl<'a, S> PaintOverlay<'a, S> {
	pub fn new(inner: &'a S, paint: LanePaint<'a>) -> Self {
		Self { inner, paint }
	}
}

pub struct PaintOverlayColumn<'a, S: LaneSource + 'a, A: Attribute> {
	inner: S::Column<'a, A>,
	forced: Option<A::Value<'a>>,
}

impl<'a, S: LaneSource, A: Attribute> LaneColumn<'a, A> for PaintOverlayColumn<'a, S, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		match self.forced {
			Some(forced) => Some(forced),
			None => self.inner.try_get(lane),
		}
	}
}

/// The forced value for the marker `A`: the lane paint where `A` is this
/// crate's fill or stroke marker, absent otherwise.
fn forced_paint<'a, A: Attribute>(paint: LanePaint<'a>) -> Option<A::Value<'a>> {
	let slot = match A::NAME {
		name if name == Fill::NAME => paint.fill,
		name if name == Stroke::NAME => paint.stroke,
		_ => None,
	}?;
	assert_eq!(
		std::any::TypeId::of::<A::Value<'static>>(),
		std::any::TypeId::of::<Option<&'static List<Graphic<'static>>>>(),
		"attribute `{}` is declared at another value type than this crate's paint form",
		A::NAME
	);
	assert_eq!(
		size_of::<A::Value<'a>>(),
		size_of::<Option<&'a List<Graphic<'a>>>>(),
		"the paint value form must span the marker's value"
	);
	// SAFETY: the census admits one value type per attribute name and panics on
	// a conflict at registration, and the asserts above re-check it, so a marker
	// named `fill` or `stroke` carries this crate's `Option<&List<Graphic>>`
	// value form at the same size.
	Some(unsafe { std::mem::transmute_copy::<Option<&'a List<Graphic>>, A::Value<'a>>(&Some(slot)) })
}

impl<'a, S: LaneSource> LaneSource for PaintOverlay<'a, S> {
	type Element = S::Element;
	type Column<'b, A: Attribute>
		= PaintOverlayColumn<'b, S, A>
	where
		Self: 'b;

	fn lane_count(&self) -> usize {
		self.inner.lane_count()
	}

	fn element(&self, lane: usize) -> Option<&S::Element> {
		self.inner.element(lane)
	}

	fn column<A: Attribute>(&self) -> PaintOverlayColumn<'_, S, A> {
		PaintOverlayColumn {
			inner: self.inner.column::<A>(),
			forced: forced_paint::<A>(self.paint),
		}
	}
}

/// Stores a paint attribute in the paint marker's owned form, the only representation paint readers accept.
pub fn set_paint_attribute(attributes: &mut ItemAttributeValues, key: &str, paint: impl IntoGraphicList) {
	attributes.insert(key, Some(paint.into_graphic_list()));
}

/// Stores a paint attribute at a list index in the paint marker's owned form, the only representation paint readers accept.
pub fn set_paint_attribute_at<T>(list: &mut List<T>, index: usize, key: &str, paint: impl IntoGraphicList) {
	list.set_attribute(key, index, Some(paint.into_graphic_list()));
}

/// Bake the provided transform into the per-item transforms of the paint graphics stored under the
/// canonical `List<Graphic>` fill and stroke attributes.
pub fn bake_paint_transforms(attributes: &mut ItemAttributeValues, transform: DAffine2) {
	fn bake_graphic_paint_transform(graphics: &mut List<Graphic>, transform: DAffine2) {
		for item_transform in graphics.iter_attribute_values_mut_or_default::<DAffine2>(ATTR_TRANSFORM) {
			*item_transform = transform * *item_transform;
		}
		for graphic in graphics.iter_element_values_mut() {
			if let Graphic::Graphic(list) = graphic {
				bake_graphic_paint_transform(list, transform);
			}
		}
	}

	for paint_key in [ATTR_FILL, ATTR_STROKE] {
		if let Some(Some(graphics)) = attributes.get_mut::<Option<List<Graphic>>>(paint_key) {
			bake_graphic_paint_transform(graphics, transform);
		}
	}
}

#[cfg(test)]
mod run_tests {
	use super::*;
	use crate::graphic::run_to_legacy_list;
	use crate::graphic::test_support::unit_square_at;
	use core_types::Color;
	use core_types::record::{FieldWrite, RunBuilder, RunView, element_write_hashed};
	use glam::DVec2;

	#[test]
	fn a_run_serves_the_parked_paint_reference() {
		let paint = List::new_from_element(Graphic::Color(Color::BLACK));
		let vector = unit_square_at(DVec2::ZERO);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&arena, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)], 1).unwrap();
		let lane = builder.push(vector.clone()).unwrap();
		builder.attr::<Fill>(lane, Some(&paint));
		let item = builder.finish();
		let run = RunView::<Vector>::new(&item).expect("the run holds vector elements");

		assert_eq!(run.attr::<Fill>(0), Some(&paint));
		assert_eq!(paint_graphics::<Fill, _>(&run, 0), Some(&paint));
		assert_eq!(paint_graphics::<Stroke, _>(&run, 0), None);

		let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");
		assert_eq!(paint_graphics::<Fill, _>(&legacy, 0), paint_graphics::<Fill, _>(&run, 0));
	}
}
