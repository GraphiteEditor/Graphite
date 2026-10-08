use super::TypesettingConfig;
use super::text_context::decoration_rects;
use core_types::list::{Item, List};
use core_types::{ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_TEXT_FRAME, ATTR_TRANSFORM};
use glam::{DAffine2, DVec2};
use parley::GlyphRun;
use skrifa::GlyphId;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as ReadFontsRef;
use skrifa::{MetadataProvider, OutlineGlyph};
use vector_types::kurbo::{Affine, BezPath, Point, Rect, Shape};
use vector_types::vector::{Vector, VectorExt};

pub struct PathBuilder {
	origin: DVec2,
	/// Contours of the glyph currently being drawn, accumulated as a single path.
	glyph_bezpath: BezPath,
	pub vector_list: List<Vector>,
	/// Per-glyph AABBs collected in single-item mode, published as `ATTR_EDITOR_CLICK_TARGET` in `finalize()`.
	merged_click_target_bboxes: Vec<[DVec2; 2]>,
	/// Per-glyph baselines, parallel to `merged_click_target_bboxes`. Groups glyphs by line for the widening pass.
	merged_click_target_baselines: Vec<f64>,
	/// Per-glyph AABBs in glyph-local space (multi-item mode), widened in `finalize()` to fill gaps.
	per_glyph_bboxes: Vec<Option<[DVec2; 2]>>,
	/// The signed area of everything drawn into the merged item so far, whose sign is the glyphs' winding direction.
	glyph_signed_area: f64,
	/// Decoration rectangles held back while drawing, so they can be wound to match the glyphs once those are all in.
	buffered_decorations: Vec<Rect>,
	/// Decoration items held back while drawing in per-glyph mode, so glyph *i* stays item *i*.
	buffered_decoration_items: Vec<Item<Vector>>,
	/// Text frame size, stamped per item as `ATTR_EDITOR_TEXT_FRAME` relative to each item's origin.
	text_frame_size: DVec2,
	/// First glyph's baseline offset (pre-height-filter). Used for the empty placeholder item so
	/// `local_transforms` stays stable when all glyphs are clipped during a resize drag.
	first_glyph_offset: DVec2,
	scale: f64,
}

impl PathBuilder {
	pub fn new(per_glyph_items: bool, scale: f64, text_frame_size: DVec2, first_glyph_offset: DVec2) -> Self {
		Self {
			glyph_bezpath: BezPath::new(),
			vector_list: if per_glyph_items { List::new() } else { List::new_from_element(Vector::default()) },
			merged_click_target_bboxes: Vec::new(),
			merged_click_target_baselines: Vec::new(),
			per_glyph_bboxes: Vec::new(),
			glyph_signed_area: 0.,
			buffered_decorations: Vec::new(),
			buffered_decoration_items: Vec::new(),
			text_frame_size,
			first_glyph_offset,
			scale,
			origin: DVec2::default(),
		}
	}

	fn point(&self, x: f32, y: f32) -> Point {
		Point::new((self.origin.x + x as f64) * self.scale, (self.origin.y - y as f64) * self.scale)
	}

	#[allow(clippy::too_many_arguments)]
	fn draw_glyph(
		&mut self,
		glyph: &OutlineGlyph<'_>,
		size: f32,
		normalized_coords: &[NormalizedCoord],
		glyph_offset: DVec2,
		style_skew: Option<DAffine2>,
		skew: DAffine2,
		per_glyph_items: bool,
	) -> bool {
		let location_ref = LocationRef::new(normalized_coords);
		let settings = DrawSettings::unhinted(Size::new(size), location_ref);
		glyph.draw(settings, self).unwrap();
		let has_geometry = !self.glyph_bezpath.is_empty();

		// Apply transforms in correct order: style-based skew first, then user-requested skew
		// This ensures font synthesis (italic) is applied before user transformations
		if let Some(style_skew) = style_skew {
			self.glyph_bezpath.apply_affine(Affine::new(style_skew.to_cols_array()));
		}
		self.glyph_bezpath.apply_affine(Affine::new(skew.to_cols_array()));

		let glyph_bbox = bezpath_bounding_box(&self.glyph_bezpath);

		if per_glyph_items {
			// Frame in item-local space: top-left at `-glyph_offset` so the item transform cancels it
			// back to the layer-local frame origin, regardless of which glyph survived
			let frame_in_item_local = DAffine2::from_scale_angle_translation(self.text_frame_size, 0., -glyph_offset);

			let item = Item::new_from_element(Vector::from_bezpath(core::mem::take(&mut self.glyph_bezpath)))
				.with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(glyph_offset))
				.with_attribute(ATTR_EDITOR_TEXT_FRAME, frame_in_item_local);
			self.vector_list.push(item);

			// Defer click target creation to `finalize()` where adjacent AABBs get widened
			self.per_glyph_bboxes.push(glyph_bbox);
		} else {
			self.glyph_signed_area += self.glyph_bezpath.area();

			// Unwrapping here is ok because `self.vector_list` is initialized with a single `List<Vector>` item
			self.vector_list.element_mut(0).unwrap().append_bezpath(core::mem::take(&mut self.glyph_bezpath));

			if let Some(bbox) = glyph_bbox {
				self.merged_click_target_bboxes.push(bbox);
				self.merged_click_target_baselines.push(glyph_offset.y);
			}
		}

		has_geometry
	}

	pub fn render_glyph_run(&mut self, glyph_run: &GlyphRun<'_, ()>, letter_tilt: f64, per_glyph_items: bool, x_offset: f32, space_extra: f32) {
		let mut run_x = glyph_run.offset() + x_offset;
		let run_y = glyph_run.baseline();

		let run = glyph_run.run();

		// User-requested letter tilt applied around baseline to avoid vertical displacement
		// Translation ensures rotation point is at the baseline, not origin
		let skew = if per_glyph_items {
			DAffine2::from_cols_array(&[1., 0., -letter_tilt.to_radians().tan(), 1., 0., 0.])
		} else {
			DAffine2::from_translation(DVec2::new(0., run_y as f64))
				* DAffine2::from_cols_array(&[1., 0., -letter_tilt.to_radians().tan(), 1., 0., 0.])
				* DAffine2::from_translation(DVec2::new(0., -run_y as f64))
		};

		let synthesis = run.synthesis();

		// Font synthesis (e.g., synthetic italic) applied separately from user transforms
		// This preserves the distinction between font styling and user transformations
		let style_skew = synthesis.skew().map(|angle| {
			if per_glyph_items {
				DAffine2::from_cols_array(&[1., 0., -angle.to_radians().tan() as f64, 1., 0., 0.])
			} else {
				DAffine2::from_translation(DVec2::new(0., run_y as f64))
					* DAffine2::from_cols_array(&[1., 0., -angle.to_radians().tan() as f64, 1., 0., 0.])
					* DAffine2::from_translation(DVec2::new(0., -run_y as f64))
			}
		});

		let font = run.font();
		let font_size = run.font_size();

		let normalized_coords = run.normalized_coords().iter().map(|coord| NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();

		// TODO: This can be cached for better performance
		let font_collection_ref = font.data.as_ref();
		let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
		let outlines = font_ref.outline_glyphs();

		for glyph in glyph_run.glyphs() {
			let glyph_offset = DVec2::new((run_x + glyph.x) as f64, (run_y - glyph.y) as f64);
			run_x += glyph.advance;

			let glyph_id = GlyphId::from(glyph.id);
			if let Some(glyph_outline) = outlines.get(glyph_id) {
				if !per_glyph_items {
					self.origin = glyph_offset;
				}
				let drew_geometry = self.draw_glyph(&glyph_outline, font_size, &normalized_coords, glyph_offset, style_skew, skew, per_glyph_items);

				if !drew_geometry && space_extra != 0. && glyph.advance > 0. {
					run_x += space_extra;
				}
			}
		}
	}

	pub fn render_decoration_run(&mut self, glyph_run: &GlyphRun<'_, ()>, typesetting: TypesettingConfig, per_glyph_items: bool, x_offset: f32, space_extra: f32, run_spaces: usize) {
		for rect in decoration_rects(glyph_run, x_offset, space_extra, run_spaces, typesetting) {
			if per_glyph_items {
				// Item-local geometry starts at the origin like a glyph's, so the layer-space offset rides only on the transform.
				let scaled = Rect::new(0., 0., (rect.x1 - rect.x0) * self.scale, (rect.y1 - rect.y0) * self.scale);
				let translation = DVec2::new(rect.x0, rect.y0);
				let frame = DAffine2::from_scale_angle_translation(self.text_frame_size, 0., -translation);
				self.buffered_decoration_items.push(
					Item::new_from_element(Vector::from_bezpath(scaled.to_path(0.)))
						.with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(translation))
						.with_attribute(ATTR_EDITOR_TEXT_FRAME, frame),
				);
			} else {
				self.buffered_decorations.push(Rect::new(rect.x0 * self.scale, rect.y0 * self.scale, rect.x1 * self.scale, rect.y1 * self.scale));
			}
		}
	}

	pub fn finalize(mut self) -> List<Vector> {
		// Empty list = all glyphs clipped by height. Create a placeholder with the same item-0
		// transform a populated list would have so `local_transforms` stays stable mid-drag.
		// TODO: Remove this hack and move the attribute up to the parent return value when <https://github.com/GraphiteEditor/Graphite/issues/3779> is done.
		if self.vector_list.is_empty() {
			let frame_in_item_local = DAffine2::from_scale_angle_translation(self.text_frame_size, 0., -self.first_glyph_offset);
			let item = Item::new_from_element(Vector::default())
				.with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(self.first_glyph_offset))
				.with_attribute(ATTR_EDITOR_TEXT_FRAME, frame_in_item_local);
			self.vector_list.push(item);
		}

		// Widen per-glyph AABBs to close horizontal gaps, then publish as click targets
		if !self.per_glyph_bboxes.is_empty() {
			// Project glyph-local AABBs into layer-local for the widening pass
			let entries: Vec<(usize, DVec2, [DVec2; 2])> = self
				.per_glyph_bboxes
				.iter()
				.enumerate()
				.filter_map(|(index, bbox)| {
					let bbox = (*bbox)?;
					let offset = self.vector_list.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, index).translation;
					Some((index, offset, [bbox[0] + offset, bbox[1] + offset]))
				})
				.collect();

			let mut layer_bboxes: Vec<[DVec2; 2]> = entries.iter().map(|entry| entry.2).collect();
			let baselines: Vec<f64> = entries.iter().map(|entry| entry.1.y).collect();
			widen_horizontal_gaps(&mut layer_bboxes, &baselines);

			// Project back to glyph-local and stamp as click targets
			for (entry, widened) in entries.iter().zip(layer_bboxes.iter()) {
				let glyph_local = [widened[0] - entry.1, widened[1] - entry.1];
				let rect = rectangle_bezpath(glyph_local[0], glyph_local[1]);
				self.vector_list.set_attribute(ATTR_EDITOR_CLICK_TARGET, entry.0, Vector::from_bezpath(rect));
			}
		}

		// Glyph separation off: widen the accumulated AABBs and bundle as one override `Vector`
		if !self.merged_click_target_bboxes.is_empty() {
			let mut bboxes = self.merged_click_target_bboxes;
			widen_horizontal_gaps(&mut bboxes, &self.merged_click_target_baselines);

			let mut widened_bezpath = BezPath::new();
			for [min, max] in &bboxes {
				widened_bezpath.extend(rectangle_bezpath(*min, *max));
			}
			self.vector_list.set_attribute(ATTR_EDITOR_CLICK_TARGET, 0, Vector::from_bezpath(widened_bezpath));
		}

		// The buffered decorations join the compound wound to match it, so under the nonzero fill rule they paint over
		// the glyphs instead of cancelling out through them. A text with no glyphs has no winding to match.
		if !self.buffered_decorations.is_empty() {
			let glyph_sign = self.glyph_signed_area.signum();
			let compound = self.vector_list.element_mut(0).unwrap();
			for rect in core::mem::take(&mut self.buffered_decorations) {
				let mut path = rect.to_path(0.);
				if glyph_sign != 0. && path.area().signum() != glyph_sign {
					path = path.reverse_subpaths();
				}
				compound.append_bezpath(path);
			}
		}

		for item in core::mem::take(&mut self.buffered_decoration_items) {
			self.vector_list.push(item);
			self.per_glyph_bboxes.push(None);
		}

		// Fill in text frame for items that don't have one yet (single-item mode, where item 0 = identity)
		let frame = DAffine2::from_scale(self.text_frame_size);
		for index in 0..self.vector_list.len() {
			if self.vector_list.attribute::<DAffine2>(ATTR_EDITOR_TEXT_FRAME, index).is_none() {
				self.vector_list.set_attribute(ATTR_EDITOR_TEXT_FRAME, index, frame);
			}
		}

		self.vector_list
	}
}

/// Widen AABBs horizontally so same-line neighbors fill inter-glyph gaps.
/// The shorter glyph (higher min.y) widens toward its taller neighbor; equal heights split the gap.
/// Assumes input is in reading order. Linear runtime.
fn widen_horizontal_gaps(bboxes: &mut [[DVec2; 2]], baselines: &[f64]) {
	for i in 0..bboxes.len().saturating_sub(1) {
		// Skip cross-line pairs (loose epsilon since baselines come from layout floats)
		if (baselines[i] - baselines[i + 1]).abs() > 1e-4 {
			continue;
		}

		let gap = bboxes[i + 1][0].x - bboxes[i][1].x;
		if gap <= 0. {
			continue;
		}

		let left_top = bboxes[i][0].y;
		let right_top = bboxes[i + 1][0].y;

		if left_top > right_top {
			bboxes[i][1].x += gap;
		} else if right_top > left_top {
			bboxes[i + 1][0].x -= gap;
		} else {
			let half = gap / 2.;
			bboxes[i][1].x += half;
			bboxes[i + 1][0].x -= half;
		}
	}
}

fn bezpath_bounding_box(bezpath: &BezPath) -> Option<[DVec2; 2]> {
	if bezpath.is_empty() {
		return None;
	}

	let rect = bezpath.bounding_box();
	Some([DVec2::new(rect.x0, rect.y0), DVec2::new(rect.x1, rect.y1)])
}

fn rectangle_bezpath(corner1: DVec2, corner2: DVec2) -> BezPath {
	Rect::new(corner1.x, corner1.y, corner2.x, corner2.y).to_path(0.)
}

impl OutlinePen for PathBuilder {
	fn move_to(&mut self, x: f32, y: f32) {
		self.glyph_bezpath.move_to(self.point(x, y));
	}

	fn line_to(&mut self, x: f32, y: f32) {
		self.glyph_bezpath.line_to(self.point(x, y));
	}

	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		self.glyph_bezpath.quad_to(self.point(x1, y1), self.point(x2, y2));
	}

	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		self.glyph_bezpath.curve_to(self.point(x1, y1), self.point(x2, y2), self.point(x3, y3));
	}

	fn close(&mut self) {
		self.glyph_bezpath.close_path();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The signed area of each closed subpath, so a test can tell whether two shapes wind the same way.
	fn subpath_areas(path: &BezPath) -> Vec<f64> {
		let mut areas = Vec::new();
		let mut points = Vec::new();
		for element in path.iter() {
			match element {
				vector_types::kurbo::PathEl::MoveTo(point) => {
					if points.len() >= 3 {
						areas.push(shoelace(&points));
					}
					points = vec![DVec2::new(point.x, point.y)];
				}
				vector_types::kurbo::PathEl::LineTo(point) => points.push(DVec2::new(point.x, point.y)),
				vector_types::kurbo::PathEl::ClosePath => {
					if points.len() >= 3 {
						areas.push(shoelace(&points));
					}
					points.clear();
				}
				_ => {}
			}
		}
		if points.len() >= 3 {
			areas.push(shoelace(&points));
		}
		areas
	}

	fn shoelace(points: &[DVec2]) -> f64 {
		points.iter().enumerate().fold(0., |sum, (index, &point)| {
			let next = points[(index + 1) % points.len()];
			sum + point.x * next.y - next.x * point.y
		}) / 2.
	}

	/// A triangle wound counter-clockwise (positive area) or clockwise (negative area).
	fn triangle(counter_clockwise: bool) -> BezPath {
		let mut path = BezPath::new();
		let (a, b, c) = (Point::new(0., 0.), Point::new(10., 0.), Point::new(0., 10.));
		if counter_clockwise {
			path.move_to(a);
			path.line_to(b);
			path.line_to(c);
		} else {
			path.move_to(a);
			path.line_to(c);
			path.line_to(b);
		}
		path.close_path();
		path
	}

	fn finalized_with_glyph_and_decoration(glyph: BezPath) -> List<Vector> {
		let mut builder = PathBuilder::new(false, 1., DVec2::ONE, DVec2::ZERO);
		builder.glyph_signed_area += glyph.area();
		builder.vector_list.element_mut(0).unwrap().append_bezpath(glyph);
		builder.buffered_decorations.push(Rect::new(2., 4., 8., 5.));
		builder.finalize()
	}

	fn output_subpath_areas(output: &List<Vector>) -> Vec<f64> {
		let vector = output.element(0).expect("a merged text always has its compound item");
		vector.stroke_bezpath_iter().flat_map(|path| subpath_areas(&path)).collect()
	}

	#[test]
	fn a_decoration_winds_with_counter_clockwise_glyphs() {
		let areas = output_subpath_areas(&finalized_with_glyph_and_decoration(triangle(true)));
		assert_eq!(areas.len(), 2, "the glyph and its decoration should both be present");
		assert!(areas.iter().all(|&area| area > 0.), "a counter-clockwise glyph must gain a counter-clockwise decoration, got {areas:?}");
	}

	#[test]
	fn a_decoration_winds_with_clockwise_glyphs() {
		let areas = output_subpath_areas(&finalized_with_glyph_and_decoration(triangle(false)));
		assert_eq!(areas.len(), 2, "the glyph and its decoration should both be present");
		assert!(areas.iter().all(|&area| area < 0.), "a clockwise glyph must gain a clockwise decoration, got {areas:?}");
	}

	#[test]
	fn per_glyph_decoration_items_land_on_the_run() {
		use crate::text_context::{TextContext, decoration_rects, for_each_styled_glyph_run};
		use crate::{FALLBACK_FONT_RESOURCE, TypesettingConfig};

		let typesetting = TypesettingConfig {
			underline: true,
			..TypesettingConfig::default()
		};
		let layout = TextContext::with_thread_local(|ctx| ctx.layout_text("Hi", &FALLBACK_FONT_RESOURCE, typesetting)).expect("layout should succeed");

		let mut expected = Vec::new();
		for_each_styled_glyph_run(&layout, "Hi", typesetting, |run, _, space_extra, run_spaces| {
			expected.extend(decoration_rects(run, 50., space_extra, run_spaces, typesetting));
		});
		assert_eq!(expected.len(), 1, "the laid-out text should be one run");

		let mut builder = PathBuilder::new(true, 1., DVec2::ONE, DVec2::ZERO);
		// A nonzero x offset proves the item transform isn't applied on top of absolute geometry.
		// Glyphs are drawn first, exactly like the per-glyph text pipeline does, so no empty placeholder appears.
		for_each_styled_glyph_run(&layout, "Hi", typesetting, |run, _, space_extra, run_spaces| {
			builder.render_glyph_run(run, 0., true, 50., space_extra);
			builder.render_decoration_run(run, typesetting, true, 50., space_extra, run_spaces);
		});
		let output = builder.finalize();
		assert_eq!(output.len(), 3, "two glyphs plus one decoration item, got {}", output.len());

		let vector = output.element(2).expect("the decoration item should come after its glyphs");
		let offset = output.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM, 2).translation;
		let [min, max] = vector.bounding_box().expect("the decoration should have bounds");

		// World rect = item offset + item-local geometry. Absolute geometry here would count the offset twice.
		let world = [min + offset, max + offset];
		let wanted = [DVec2::new(expected[0].x0, expected[0].y0), DVec2::new(expected[0].x1, expected[0].y1)];
		assert!((world[0] - wanted[0]).length() < 1e-6, "got {world:?} for {wanted:?}");
		assert!((world[1] - wanted[1]).length() < 1e-6, "got {world:?} for {wanted:?}");
	}
}
