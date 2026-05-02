use core_types::table::{Table, TableRow};
use core_types::{ATTR_EDITOR_CLICK_TARGET, ATTR_EDITOR_TEXT_FRAME, ATTR_TRANSFORM};
use glam::{DAffine2, DVec2};
use parley::GlyphRun;
use skrifa::GlyphId;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::FontRef as ReadFontsRef;
use skrifa::{MetadataProvider, OutlineGlyph};
use vector_types::subpath::{ManipulatorGroup, Subpath};
use vector_types::vector::{PointId, Vector};

pub struct PathBuilder {
	current_subpath: Subpath<PointId>,
	origin: DVec2,
	glyph_subpaths: Vec<Subpath<PointId>>,
	pub vector_table: Table<Vector>,
	/// Per-glyph bbox rectangles collected in single-item mode, published as `ATTR_EDITOR_CLICK_TARGET` in `finalize()`.
	merged_click_target_subpaths: Vec<Subpath<PointId>>,
	/// Text frame size, stamped per item as `ATTR_EDITOR_TEXT_FRAME` relative to each item's origin.
	text_frame_size: DVec2,
	/// First glyph's baseline offset (pre-height-filter). Used for the empty placeholder item so
	/// `local_transforms` stays stable when all glyphs are clipped during a resize drag.
	first_glyph_offset: DVec2,
	scale: f64,
	id: PointId,
}

impl PathBuilder {
	pub fn new(per_glyph_items: bool, scale: f64, text_frame_size: DVec2, first_glyph_offset: DVec2) -> Self {
		Self {
			current_subpath: Subpath::new(Vec::new(), false),
			glyph_subpaths: Vec::new(),
			vector_table: if per_glyph_items { Table::new() } else { Table::new_from_element(Vector::default()) },
			merged_click_target_subpaths: Vec::new(),
			text_frame_size,
			first_glyph_offset,
			scale,
			id: PointId::ZERO,
			origin: DVec2::default(),
		}
	}

	fn point(&self, x: f32, y: f32) -> DVec2 {
		DVec2::new(self.origin.x + x as f64, self.origin.y - y as f64) * self.scale
	}

	#[allow(clippy::too_many_arguments)]
	fn draw_glyph(&mut self, glyph: &OutlineGlyph<'_>, size: f32, normalized_coords: &[NormalizedCoord], glyph_offset: DVec2, style_skew: Option<DAffine2>, skew: DAffine2, per_glyph_items: bool) {
		let location_ref = LocationRef::new(normalized_coords);
		let settings = DrawSettings::unhinted(Size::new(size), location_ref);
		glyph.draw(settings, self).unwrap();

		// Apply transforms in correct order: style-based skew first, then user-requested skew
		// This ensures font synthesis (italic) is applied before user transformations
		for glyph_subpath in &mut self.glyph_subpaths {
			if let Some(style_skew) = style_skew {
				glyph_subpath.apply_transform(style_skew);
			}

			glyph_subpath.apply_transform(skew);
		}

		// Bounding-box rectangle for click-targeting the glyph's full bounds (not just the letterform)
		let glyph_bbox_rectangle = subpaths_bounding_box(&self.glyph_subpaths).map(|[min, max]| Subpath::new_rectangle(min, max));

		if per_glyph_items {
			// Frame in item-local space: top-left at `-glyph_offset` so the item transform cancels it
			// back to the layer-local frame origin, regardless of which glyph survived
			let frame_in_item_local = DAffine2::from_scale_angle_translation(self.text_frame_size, 0., -glyph_offset);

			let item = TableRow::new_from_element(Vector::from_subpaths(core::mem::take(&mut self.glyph_subpaths), false))
				.with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(glyph_offset))
				.with_attribute(ATTR_EDITOR_TEXT_FRAME, frame_in_item_local);
			let item = match glyph_bbox_rectangle {
				Some(rect) => item.with_attribute(ATTR_EDITOR_CLICK_TARGET, Vector::from_subpaths([rect], false)),
				None => item,
			};
			self.vector_table.push(item);
		} else {
			for subpath in self.glyph_subpaths.drain(..) {
				// Unwrapping here is ok because `self.vector_table` is initialized with a single `Table<Vector>` item
				self.vector_table.element_mut(0).unwrap().append_subpath(subpath, false);
			}
			if let Some(rect) = glyph_bbox_rectangle {
				self.merged_click_target_subpaths.push(rect);
			}
		}
	}

	pub fn render_glyph_run(&mut self, glyph_run: &GlyphRun<'_, ()>, tilt: f64, per_glyph_items: bool) {
		let mut run_x = glyph_run.offset();
		let run_y = glyph_run.baseline();

		let run = glyph_run.run();

		// User-requested tilt applied around baseline to avoid vertical displacement
		// Translation ensures rotation point is at the baseline, not origin
		let skew = if per_glyph_items {
			DAffine2::from_cols_array(&[1., 0., -tilt.to_radians().tan(), 1., 0., 0.])
		} else {
			DAffine2::from_translation(DVec2::new(0., run_y as f64))
				* DAffine2::from_cols_array(&[1., 0., -tilt.to_radians().tan(), 1., 0., 0.])
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
				self.draw_glyph(&glyph_outline, font_size, &normalized_coords, glyph_offset, style_skew, skew, per_glyph_items);
			}
		}
	}

	pub fn finalize(mut self) -> Table<Vector> {
		// Empty table = all glyphs clipped by height. Create a placeholder with the same item-0
		// transform a populated table would have so `local_transforms` stays stable mid-drag.
		// TODO: Remove this hack and move the attribute up to the parent return value when <https://github.com/GraphiteEditor/Graphite/issues/3779> is done.
		if self.vector_table.is_empty() {
			let frame_in_item_local = DAffine2::from_scale_angle_translation(self.text_frame_size, 0., -self.first_glyph_offset);
			let item = TableRow::new_from_element(Vector::default())
				.with_attribute(ATTR_TRANSFORM, DAffine2::from_translation(self.first_glyph_offset))
				.with_attribute(ATTR_EDITOR_TEXT_FRAME, frame_in_item_local);
			self.vector_table.push(item);
		}

		// With "Separate Glyph Elements" inactive, combine the accumulated per-glyph AABBs as one override `Vector`
		if !self.merged_click_target_subpaths.is_empty() {
			self.vector_table
				.set_attribute(ATTR_EDITOR_CLICK_TARGET, 0, Vector::from_subpaths(self.merged_click_target_subpaths, false));
		}

		// Fill in text frame for items that don't have one yet (single-item mode, where item 0 = identity)
		let frame = DAffine2::from_scale(self.text_frame_size);
		for index in 0..self.vector_table.len() {
			if self.vector_table.attribute::<DAffine2>(ATTR_EDITOR_TEXT_FRAME, index).is_none() {
				self.vector_table.set_attribute(ATTR_EDITOR_TEXT_FRAME, index, frame);
			}
		}

		self.vector_table
	}
}

fn subpaths_bounding_box(subpaths: &[Subpath<PointId>]) -> Option<[DVec2; 2]> {
	subpaths
		.iter()
		.filter_map(|subpath| subpath.bounding_box())
		.reduce(|[a_min, a_max], [b_min, b_max]| [a_min.min(b_min), a_max.max(b_max)])
}

impl OutlinePen for PathBuilder {
	fn move_to(&mut self, x: f32, y: f32) {
		if !self.current_subpath.is_empty() {
			self.glyph_subpaths.push(std::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
		}
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(self.point(x, y), self.id.next_id()));
	}

	fn line_to(&mut self, x: f32, y: f32) {
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_anchor_with_id(self.point(x, y), self.id.next_id()));
	}

	fn quad_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
		let [handle, anchor] = [self.point(x1, y1), self.point(x2, y2)];
		self.current_subpath.last_manipulator_group_mut().unwrap().out_handle = Some(handle);
		self.current_subpath.push_manipulator_group(ManipulatorGroup::new_with_id(anchor, None, None, self.id.next_id()));
	}

	fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
		let [handle1, handle2, anchor] = [self.point(x1, y1), self.point(x2, y2), self.point(x3, y3)];
		self.current_subpath.last_manipulator_group_mut().unwrap().out_handle = Some(handle1);
		self.current_subpath
			.push_manipulator_group(ManipulatorGroup::new_with_id(anchor, Some(handle2), None, self.id.next_id()));
	}

	fn close(&mut self) {
		self.current_subpath.set_closed(true);
		self.glyph_subpaths.push(std::mem::replace(&mut self.current_subpath, Subpath::new(Vec::new(), false)));
	}
}
