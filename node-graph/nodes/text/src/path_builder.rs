use core_types::table::{Table, TableRow};
use glam::{DAffine2, DVec2};
use kurbo::{PathSeg, Point};
use skrifa::instance::{NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlineGlyph, OutlinePen};
use skrifa::raw::FontRef as ReadFontsRef;
use skrifa::{GlyphId, MetadataProvider};
use vector_types::{Subpath, Vector};

pub struct PathBuilder<Upstream: Default + 'static> {
	vector_table: Table<Vector<Upstream>>,
	current_segments: Vec<PathSeg>,
	glyph_subpaths: Vec<Subpath<vector_types::vector::PointId>>,
	current_point: Point,
	is_text_on_path: bool,
	scale: f64,
	glyph_index: u64,
}

impl<Upstream: Default + 'static> PathBuilder<Upstream> {
	pub fn new(is_text_on_path: bool, scale: f64) -> Self {
		Self {
			vector_table: Table::new(),
			current_segments: Vec::new(),
			glyph_subpaths: Vec::new(),
			current_point: Point::ZERO,
			is_text_on_path,
			scale,
			glyph_index: 0,
		}
	}

	fn point(&self, x: f32, y: f32) -> Point {
		Point::new(x as f64, -y as f64)
	}

	fn outline_glyph(&mut self, glyph: &OutlineGlyph<'_>, size: f32, normalized_coords: &[NormalizedCoord]) -> bool {
		self.glyph_subpaths.clear();
		self.current_segments.clear();
		self.current_point = Point::ZERO;

		let settings = DrawSettings::unhinted(Size::new(size), normalized_coords);
		if let Err(e) = glyph.draw(settings, self) {
			log::error!("Failed to draw glyph: {:?}", e);
			return false;
		}

		if !self.current_segments.is_empty() {
			self.glyph_subpaths.push(Subpath::from_beziers(&self.current_segments, false));
			self.current_segments.clear();
		}

		true
	}

	pub fn draw_glyph(&mut self, glyph: &OutlineGlyph<'_>, size: f32, normalized_coords: &[NormalizedCoord], style_skew: Option<DAffine2>, final_transform: DAffine2, per_glyph_instances: bool) {
		if !self.outline_glyph(glyph, size, normalized_coords) {
			return;
		}

		let transform = if self.is_text_on_path {
			final_transform
		} else {
			final_transform * DAffine2::from_scale(DVec2::splat(self.scale))
		};
		let transform = if let Some(skew) = style_skew { transform * skew } else { transform };

		let mut vector = Vector::from_subpaths(self.glyph_subpaths.clone(), false);
		vector.transform(transform);
		if per_glyph_instances {
			self.vector_table.push(TableRow::new_from_element(vector));
		} else if self.vector_table.is_empty() {
			self.vector_table = Table::new_from_element(vector);
		} else {
			let current_vector = self.vector_table.iter_mut().next().unwrap();
			current_vector.element.concat(&vector, DAffine2::IDENTITY, self.glyph_index);
		}
		self.glyph_index += 1;
	}

	pub fn draw_glyph_with_mapping(&mut self, glyph: &OutlineGlyph<'_>, size: f32, normalized_coords: &[NormalizedCoord], style_skew: Option<DAffine2>, mapping_function: impl Fn(DVec2) -> DVec2) {
		if !self.outline_glyph(glyph, size, normalized_coords) {
			return;
		}

		let subpaths = std::mem::take(&mut self.glyph_subpaths)
			.into_iter()
			.map(|mut subpath| {
				for manipulator_group in subpath.manipulator_groups_mut() {
					let transform_point = |point| {
						let point = style_skew.map_or(point, |skew| skew.transform_point2(point));
						mapping_function(point)
					};
					manipulator_group.anchor = transform_point(manipulator_group.anchor);
					manipulator_group.in_handle = manipulator_group.in_handle.map(transform_point);
					manipulator_group.out_handle = manipulator_group.out_handle.map(transform_point);
				}
				subpath
			})
			.collect::<Vec<_>>();

		self.vector_table.push(TableRow::new_from_element(Vector::from_subpaths(subpaths, false)));
	}

	pub fn render_glyph_run(&mut self, glyph_run: &parley::GlyphRun<'_, ()>, tilt: f64, per_glyph_instances: bool) {
		let run = glyph_run.run();
		let mut run_x = glyph_run.offset();
		let run_y = glyph_run.baseline();

		let synthesis = run.synthesis();
		let style_skew = synthesis.skew().map(|angle| {
			let skew = DAffine2::from_cols_array(&[1., 0., -(angle as f64).to_radians().tan(), 1., 0., 0.]);
			if per_glyph_instances || self.is_text_on_path {
				skew
			} else {
				DAffine2::from_translation(DVec2::new(0., run_y as f64)) * skew * DAffine2::from_translation(DVec2::new(0., -run_y as f64))
			}
		});
		let tilt_skew = (tilt != 0.).then(|| DAffine2::from_cols_array(&[1., 0., -tilt.to_radians().tan(), 1., 0., 0.]));

		let font = run.font();
		let font_size = run.font_size();

		let normalized_coords = run.normalized_coords().iter().map(|coord| NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();

		let font_collection_ref = font.data.as_ref();
		let font_ref = ReadFontsRef::from_index(font_collection_ref, font.index).unwrap();
		let outlines = font_ref.outline_glyphs();

		glyph_run.glyphs().for_each(|glyph| {
			let glyph_offset = DVec2::new((run_x + glyph.x) as f64, (run_y - glyph.y) as f64);
			run_x += glyph.advance;

			if let Some(glyph_outline) = outlines.get(GlyphId::from(glyph.id)) {
				let mut final_transform = DAffine2::from_translation(glyph_offset);
				if let Some(tilt_skew) = tilt_skew {
					final_transform = final_transform * tilt_skew;
				}

				self.draw_glyph(&glyph_outline, font_size, &normalized_coords, style_skew, final_transform, per_glyph_instances);
			}
		});
	}

	pub fn finalize(mut self) -> Table<Vector<Upstream>> {
		if self.vector_table.is_empty() {
			self.vector_table = Table::new_from_element(Vector::default())
		}
		self.vector_table
	}
}

impl<Upstream: Default + 'static> OutlinePen for PathBuilder<Upstream> {
	fn move_to(&mut self, x: f32, y: f32) {
		if !self.current_segments.is_empty() {
			self.glyph_subpaths.push(Subpath::from_beziers(&self.current_segments, false));
			self.current_segments.clear();
		}
		self.current_point = self.point(x, y);
	}

	fn line_to(&mut self, x: f32, y: f32) {
		let p = self.point(x, y);
		self.current_segments.push(PathSeg::Line(kurbo::Line::new(self.current_point, p)));
		self.current_point = p;
	}

	fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
		let p1 = self.point(cx0, cy0);
		let p2 = self.point(x, y);
		self.current_segments.push(PathSeg::Quad(kurbo::QuadBez::new(self.current_point, p1, p2)));
		self.current_point = p2;
	}

	fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
		let p1 = self.point(cx0, cy0);
		let p2 = self.point(cx1, cy1);
		let p3 = self.point(x, y);
		self.current_segments.push(PathSeg::Cubic(kurbo::CubicBez::new(self.current_point, p1, p2, p3)));
		self.current_point = p3;
	}

	fn close(&mut self) {
		if !self.current_segments.is_empty() {
			self.glyph_subpaths.push(Subpath::from_beziers(&self.current_segments, true));
			self.current_segments.clear();
		}
	}
}
