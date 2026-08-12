//! The appearance model: an ordered list of paint passes ("coverages") stored in the `ATTR_APPEARANCE` attribute.
//! Data uniform across all covers (the paint) rides the outer `List<Coverage>` so columnar presence holds,
//! while cover-specific data rides the inner `Item<Cover>`, reusing `ATTR_TRANSFORM` for the stroke-authoring space.

use crate::graphic::{Graphic, is_paint_present};
use core_types::graphene_hash::CacheHash;
use core_types::list::{ATTR_ALIGN, ATTR_CAP, ATTR_DASH_OFFSET, ATTR_DASH_PATTERN, ATTR_JOIN, ATTR_JOIN_MITER_LIMIT, ATTR_PAINT, ATTR_TRANSFORM, ATTR_WEIGHT, Item, List};
use vector_types::vector::style::{DashPattern, Stroke};

/// The geometry-to-region operator a coverage applies before painting:
/// the interior of the geometry (fill) or the region swept along its outline (stroke).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, CacheHash)]
pub enum Cover {
	#[default]
	Fill,
	Stroke,
}

/// One paint pass of an [`Appearance`]: a [`Cover`] plus its cover-specific parameters, carried as
/// attributes on the inner item. Attributes for stroke parameters are ignored on fill coverages.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Coverage(pub Item<Cover>);

/// An item's ordered list of paint passes, stored in the `ATTR_APPEARANCE` attribute cell.
/// Earlier coverages paint first, compositing below later ones. Each row's paint is the
/// `ATTR_PAINT` attribute beside it.
#[derive(Clone, Debug, Default, PartialEq, CacheHash)]
pub struct Appearance(pub List<Coverage>);

/// Where a newly inserted coverage lands in the paint order when no same-cover coverage exists to replace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverPlacement {
	/// The front of the list, painting first (below every existing pass).
	Below,
	/// The back of the list, painting last (above every existing pass).
	Above,
}

impl Coverage {
	/// Creates a fill coverage with no parameters beyond its cover.
	pub fn new_fill() -> Self {
		Self(Item::new_from_element(Cover::Fill))
	}

	/// Creates a stroke coverage, stamping only the parameters that differ from their implicit defaults.
	pub fn new_stroke(stroke: &Stroke) -> Self {
		let defaults = Stroke::default();
		let mut item = Item::new_from_element(Cover::Stroke);

		if stroke.weight != defaults.weight {
			item.set_attribute(ATTR_WEIGHT, stroke.weight);
		}
		if !stroke.dash_lengths.is_empty() {
			item.set_attribute(ATTR_DASH_PATTERN, DashPattern::from(stroke.dash_lengths.clone()));
		}
		if stroke.dash_offset != defaults.dash_offset {
			item.set_attribute(ATTR_DASH_OFFSET, stroke.dash_offset);
		}
		if stroke.cap != defaults.cap {
			item.set_attribute(ATTR_CAP, stroke.cap);
		}
		if stroke.join != defaults.join {
			item.set_attribute(ATTR_JOIN, stroke.join);
		}
		if stroke.join_miter_limit != defaults.join_miter_limit {
			item.set_attribute(ATTR_JOIN_MITER_LIMIT, stroke.join_miter_limit);
		}
		if stroke.align != defaults.align {
			item.set_attribute(ATTR_ALIGN, stroke.align);
		}
		if stroke.transform != defaults.transform {
			item.set_attribute(ATTR_TRANSFORM, stroke.transform);
		}

		Self(item)
	}

	/// This coverage's cover.
	pub fn cover(&self) -> Cover {
		*self.0.element()
	}

	/// Extracts the stroke parameters into a [`Stroke`], falling back to the default for any absent attribute.
	/// Dash lengths are clamped to non-negative, matching what rendering accepts.
	pub fn stroke_params(&self) -> Stroke {
		let defaults = Stroke::default();
		Stroke {
			weight: self.0.attribute_cloned_or(ATTR_WEIGHT, defaults.weight),
			dash_lengths: self.0.attribute::<DashPattern>(ATTR_DASH_PATTERN).map_or(defaults.dash_lengths, DashPattern::clamped_lengths),
			dash_offset: self.0.attribute_cloned_or(ATTR_DASH_OFFSET, defaults.dash_offset),
			cap: self.0.attribute_cloned_or(ATTR_CAP, defaults.cap),
			join: self.0.attribute_cloned_or(ATTR_JOIN, defaults.join),
			join_miter_limit: self.0.attribute_cloned_or(ATTR_JOIN_MITER_LIMIT, defaults.join_miter_limit),
			align: self.0.attribute_cloned_or(ATTR_ALIGN, defaults.align),
			transform: self.0.attribute_cloned_or(ATTR_TRANSFORM, defaults.transform),
			paint_order: defaults.paint_order,
		}
	}
}

impl Appearance {
	/// Creates an appearance holding a single coverage with the given paint.
	pub fn new_single(coverage: Coverage, paint: List<Graphic>) -> Self {
		Self(List::new_from_item(Item::new_from_element(coverage).with_attribute(ATTR_PAINT, paint)))
	}

	/// The number of coverages in this appearance.
	pub fn len(&self) -> usize {
		self.0.len()
	}

	/// Whether this appearance holds no coverages, which paints nothing.
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	/// Iterates the coverages in paint order.
	pub fn covers(&self) -> impl Iterator<Item = &Coverage> {
		self.0.iter_element_values()
	}

	/// The coverage at the given index in paint order.
	pub fn cover_at(&self, index: usize) -> Option<&Coverage> {
		self.0.element(index)
	}

	/// The paint of the coverage at the given index, or `None` if the paint attribute is absent.
	pub fn paint_at(&self, index: usize) -> Option<&List<Graphic>> {
		self.0.attribute::<List<Graphic>>(ATTR_PAINT, index)
	}

	/// The index of the first coverage of the given cover in paint order.
	pub fn first_index_of(&self, cover: Cover) -> Option<usize> {
		self.covers().position(|coverage| coverage.cover() == cover)
	}

	/// Whether any coverage of the given cover exists, regardless of whether its paint draws anything.
	pub fn has_cover(&self, cover: Cover) -> bool {
		self.first_index_of(cover).is_some()
	}

	/// Whether any coverage of the given cover has paint that draws something, i.e. paint that is
	/// present and not empty. A coverage whose paint is [`Graphic::None`] exists but paints nothing.
	pub fn has_painted_cover(&self, cover: Cover) -> bool {
		self.covers()
			.enumerate()
			.any(|(index, coverage)| coverage.cover() == cover && self.paint_at(index).is_some_and(is_paint_present))
	}

	/// Replaces the first coverage of the incoming cover in place (keeping its position in the paint order),
	/// or inserts a new row at the requested end of the paint order if none exists.
	pub fn replace_or_insert(&mut self, coverage: Coverage, paint: List<Graphic>, placement: CoverPlacement) {
		if let Some(index) = self.first_index_of(coverage.cover()) {
			if let Some(element) = self.0.element_mut(index) {
				*element = coverage;
			}
			self.0.set_attribute(ATTR_PAINT, index, paint);
			return;
		}

		let mut row = Item::new_from_element(coverage);
		if is_paint_present(&paint) {
			row.set_attribute(ATTR_PAINT, paint);
		}
		match placement {
			CoverPlacement::Above => self.0.push(row),
			CoverPlacement::Below => {
				let mut reordered = List::new_from_item(row);
				reordered.extend(std::mem::take(&mut self.0));
				self.0 = reordered;
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::Color;
	use glam::{DAffine2, DVec2};
	use vector_types::vector::style::{StrokeAlign, StrokeCap, StrokeJoin};

	fn solid_paint(color: Color) -> List<Graphic> {
		List::new_from_element(Graphic::Color(List::new_from_element(color)))
	}

	fn paint_color(appearance: &Appearance, index: usize) -> Option<Color> {
		let paint = appearance.paint_at(index)?;
		let Some(Graphic::Color(colors)) = paint.element(0) else { return None };
		colors.element(0).copied()
	}

	#[test]
	fn stroke_params_survive_the_attribute_round_trip() {
		let stroke = Stroke {
			weight: 3.,
			dash_lengths: vec![4., -2.],
			dash_offset: 1.5,
			cap: StrokeCap::Round,
			join: StrokeJoin::Bevel,
			join_miter_limit: 7.,
			align: StrokeAlign::Inside,
			transform: DAffine2::from_scale(DVec2::new(2., 3.)),
			..Stroke::default()
		};

		let coverage = Coverage::new_stroke(&stroke);
		assert_eq!(coverage.cover(), Cover::Stroke);

		let extracted = coverage.stroke_params();
		assert_eq!(extracted.weight, 3.);
		assert_eq!(extracted.dash_lengths, vec![4., 0.], "negative dash lengths should clamp to zero on extraction");
		assert_eq!(extracted.dash_offset, 1.5);
		assert_eq!(extracted.cap, StrokeCap::Round);
		assert_eq!(extracted.join, StrokeJoin::Bevel);
		assert_eq!(extracted.join_miter_limit, 7.);
		assert_eq!(extracted.align, StrokeAlign::Inside);
		assert_eq!(extracted.transform, DAffine2::from_scale(DVec2::new(2., 3.)));
	}

	#[test]
	fn default_valued_stroke_parameters_elide_to_absence() {
		let coverage = Coverage::new_stroke(&Stroke::default());
		assert_eq!(coverage.0.attributes().keys().count(), 0, "default parameters should stay absent");
		assert_eq!(coverage.stroke_params(), Stroke::default(), "absent attributes should read back as the defaults");

		let coverage = Coverage::new_stroke(&Stroke::new(2.));
		let keys: Vec<_> = coverage.0.attributes().keys().collect();
		assert_eq!(keys, vec![ATTR_WEIGHT], "only the non-default weight should be stamped");
		assert_eq!(coverage.stroke_params().weight, 2.);
	}

	#[test]
	fn replace_keeps_position_and_the_other_rows_paint() {
		let mut appearance = Appearance::default();
		appearance.replace_or_insert(Coverage::new_fill(), solid_paint(Color::RED), CoverPlacement::Above);
		appearance.replace_or_insert(Coverage::new_stroke(&Stroke::new(2.)), solid_paint(Color::BLACK), CoverPlacement::Above);

		appearance.replace_or_insert(Coverage::new_fill(), solid_paint(Color::BLUE), CoverPlacement::Above);

		assert_eq!(appearance.len(), 2, "replacement should not add a row");
		assert_eq!(appearance.cover_at(0).map(Coverage::cover), Some(Cover::Fill), "the fill should keep its position");
		assert_eq!(paint_color(&appearance, 0), Some(Color::BLUE));
		assert_eq!(paint_color(&appearance, 1), Some(Color::BLACK), "the stroke row's paint should be untouched");
	}

	#[test]
	fn below_insertion_prepends_and_preserves_paint_columns() {
		let mut appearance = Appearance::default();
		appearance.replace_or_insert(Coverage::new_stroke(&Stroke::new(2.)), solid_paint(Color::BLACK), CoverPlacement::Above);
		appearance.replace_or_insert(Coverage::new_fill(), solid_paint(Color::RED), CoverPlacement::Below);

		let covers: Vec<_> = appearance.covers().map(Coverage::cover).collect();
		assert_eq!(covers, vec![Cover::Fill, Cover::Stroke], "a below-placed fill should paint before the stroke");
		assert_eq!(paint_color(&appearance, 0), Some(Color::RED));
		assert_eq!(paint_color(&appearance, 1), Some(Color::BLACK), "the existing row's paint should survive the reorder");
	}

	#[test]
	fn painted_cover_distinguishes_none_paint_from_absence() {
		let mut appearance = Appearance::default();
		appearance.replace_or_insert(Coverage::new_fill(), List::new_from_element(Graphic::None), CoverPlacement::Above);

		assert!(appearance.has_cover(Cover::Fill), "a none-painted coverage still exists");
		assert!(!appearance.has_painted_cover(Cover::Fill), "a none-painted coverage draws nothing");
		assert!(!appearance.has_cover(Cover::Stroke));
		assert!(!appearance.has_painted_cover(Cover::Stroke));

		appearance.replace_or_insert(Coverage::new_fill(), solid_paint(Color::RED), CoverPlacement::Above);
		assert!(appearance.has_painted_cover(Cover::Fill));
	}
}
