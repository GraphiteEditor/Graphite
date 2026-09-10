//! The appearance cascade level: the declared appearance read as a lane column and threaded down to the elements it reaches.

use super::Graphic;
use crate::appearance::Appearance;
use crate::markers::Appearance as AppearanceMarker;
use core_types::ATTR_TRANSFORM;
use core_types::attribute::Opacity;
use core_types::lane::{LaneColumn, LaneSource};
use core_types::list::{ItemAttributeValues, List};
use glam::DAffine2;
use vector_types::Vector;

/// Whether a normalized paint graphic list actually carries renderable paint.
/// A 0-item list, or a list whose first graphic is empty, is treated as no paint.
pub fn is_paint_present(graphic_list: &List<Graphic>) -> bool {
	graphic_list.element(0).is_some_and(|graphic| !graphic.is_empty())
}

/// Whether every lane of a vector source draws as a plain clip path: fully
/// opaque, fill absent or opaque, stroke invisible or fully transparent.
pub fn vector_can_reduce_to_clip_path<S: LaneSource<Element = Vector>>(source: &S, inherited_appearance: Option<&Appearance>) -> bool {
	(0..source.lane_count()).all(|index| {
		if source.element(index).is_none() {
			return false;
		}
		let opacity: f64 = source.attr::<Opacity>(index);

		let appearance = Appearance::cascade(source.attr::<AppearanceMarker>(index), inherited_appearance);
		let resolved = appearance.map(Appearance::fill_and_stroke).unwrap_or_default();

		let fill_opaque_or_absent = resolved
			.fill_paint
			.and_then(paint_cell_rows)
			.is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_opaque()));

		let stroke_invisible_or_transparent = resolved.stroke.as_ref().is_none_or(|stroke| !stroke.has_renderable_stroke())
			|| resolved
				.stroke_paint
				.and_then(paint_cell_rows)
				.is_none_or(|graphic_list| graphic_list.element(0).is_none_or(|graphic| graphic.is_fully_transparent()));

		opacity > 1. - f64::EPSILON && fill_opaque_or_absent && stroke_invisible_or_transparent
	})
}

/// A source's declared appearance column, resolved once for per-lane reads.
pub struct PaintColumns<'a, S: LaneSource + 'a> {
	appearance: S::Column<'a, AppearanceMarker>,
}

impl<'a, S: LaneSource> PaintColumns<'a, S> {
	pub fn new(source: &'a S) -> Self {
		Self {
			appearance: source.column::<AppearanceMarker>(),
		}
	}

	/// The lane's own declared appearance; an absent or empty cell is undeclared.
	pub fn read_appearance(&self, lane: usize) -> Option<&'a Appearance> {
		self.appearance.try_get(lane).flatten().and_then(Appearance::declared)
	}
}

/// The appearance cascade threading down the graphic levels: a lane's own
/// declared appearance wins wholesale, an undeclared lane inherits the
/// nearest ancestor's, at any depth. Only a fresh entry (a pattern's own
/// render, or any standalone render root) starts without an inherited one.
#[derive(Clone, Copy)]
pub struct PaintReach<'a> {
	/// The cascade's resolved appearance: the nearest declared one at or above this lane.
	pub appearance: Option<&'a Appearance>,
}

impl<'a> PaintReach<'a> {
	pub const NONE: Self = Self { appearance: None };

	/// The lane's effective reach: its own declared appearance wins over the inherited one.
	pub fn for_lane<S: LaneSource>(self, columns: &PaintColumns<'a, S>, index: usize) -> Self {
		Self {
			appearance: Appearance::cascade(columns.read_appearance(index), self.appearance),
		}
	}
}

/// The paint a coverage row's cell holds, in the canonical `List<Graphic>` form the paint
/// renderers consume: this crate's writers carry the list as one graphic cell, and a bare
/// cell of any other form is treated as paint that draws nothing.
pub fn paint_cell_rows<'a>(cell: &'a Graphic<'static>) -> Option<&'a List<Graphic<'static>>> {
	match cell {
		Graphic::Graphic(list) => Some(list).filter(|list| is_paint_present(list)),
		_ => None,
	}
}

/// Bake the provided transform into the per-item transforms of the paint
/// graphics inside the item's appearance coverage cells.
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

	if let Some(appearance) = attributes.get_mut::<Appearance>(crate::markers::ATTR_APPEARANCE) {
		if let Some(cells) = appearance.0.iter_attribute_values_mut::<Graphic>(crate::markers::ATTR_PAINT) {
			for cell in cells {
				if let Graphic::Graphic(list) = cell {
					bake_graphic_paint_transform(list, transform);
				}
			}
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
	fn a_run_serves_the_parked_appearance_reference() {
		use crate::appearance::Coverage;
		use core_types::lane::LaneSource;

		let appearance = Appearance::new_single(Coverage::new_fill(), Graphic::Color(Color::BLACK));
		let vector = unit_square_at(DVec2::ZERO);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&arena, element_write_hashed::<Vector>(), &[FieldWrite::of::<AppearanceMarker>(0)], 1).unwrap();
		let lane = builder.push(vector.clone()).unwrap();
		builder.attr::<AppearanceMarker>(lane, Some(&appearance));
		let item = builder.finish();
		let run = RunView::<Vector>::new(&item).expect("the run holds vector elements");

		assert_eq!(run.attr::<AppearanceMarker>(0), Some(&appearance));

		let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");
		assert_eq!(legacy.attr::<AppearanceMarker>(0), Some(&appearance));
	}

	#[test]
	fn reach_cascades_the_appearance_with_own_wins_arbitration() {
		use crate::appearance::Coverage;
		use crate::markers::ATTR_APPEARANCE;

		let own = Appearance::new_single(Coverage::new_fill(), Graphic::Color(Color::BLACK));
		let inherited = Appearance::new_single(Coverage::new_fill(), Graphic::Color(Color::WHITE));

		// Lane 0 declares its own appearance, lane 1 is padded with the empty (undeclared) one
		let mut list: List<Graphic<'static>> = List::new_from_element(Graphic::Vector(Vector::default()));
		list.push(core_types::list::Item::new_from_element(Graphic::Vector(Vector::default())));
		list.set_attribute(ATTR_APPEARANCE, 0, own.clone());

		let columns = PaintColumns::new(&list);
		let ancestor = PaintReach { appearance: Some(&inherited) };

		assert_eq!(ancestor.for_lane(&columns, 0).appearance, Some(&own), "a declared lane wins over the inherited appearance");
		assert_eq!(ancestor.for_lane(&columns, 1).appearance, Some(&inherited), "a padded lane inherits");
		assert_eq!(PaintReach::NONE.for_lane(&columns, 1).appearance, None, "no ancestor leaves an undeclared lane bare");
	}
}
