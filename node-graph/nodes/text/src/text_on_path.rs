use core_types::table::Table;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg};
use parley::PositionedLayoutItem;
use skrifa::MetadataProvider;
use skrifa::raw::FontRef as ReadFontsRef;
use vector_types::Vector;

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash, serde::Serialize, serde::Deserialize, DynAny, node_macro::ChoiceType)]
pub enum TextPathSide {
	#[default]
	Left,
	Right,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash, serde::Serialize, serde::Deserialize, DynAny, node_macro::ChoiceType)]
pub enum TextAnchor {
	#[default]
	Start,
	Middle,
	End,
}

pub struct ArcLengthLut {
	lengths: Vec<f64>,
	params: Vec<(usize, f64)>,
	segs: Vec<PathSeg>,
	pub total_length: f64,
	pub is_closed: bool,
}

impl ArcLengthLut {
	pub fn build(path: &BezPath, samples_per_segment: usize) -> Self {
		let accuracy = 1e-6;
		let mut lengths = vec![0.0_f64];
		let mut params = vec![(0_usize, 0.0_f64)];
		let mut cumulative = 0.0_f64;
		let mut cached_segs = Vec::new();

		for (seg_idx, seg) in path.segments().enumerate() {
			cached_segs.push(seg);
			let seg_len = seg.arclen(accuracy);
			for i in 1..=samples_per_segment {
				let t = i as f64 / samples_per_segment as f64;
				let sub_len = seg.subsegment(0.0..t).arclen(accuracy);
				lengths.push(cumulative + sub_len);
				params.push((seg_idx, t));
			}
			cumulative += seg_len;
		}

		let is_closed = path.elements().last() == Some(&PathEl::ClosePath);

		Self {
			lengths,
			params,
			segs: cached_segs,
			total_length: cumulative,
			is_closed,
		}
	}

	fn eval_tangent(seg: PathSeg, t: f64) -> kurbo::Vec2 {
		match seg {
			PathSeg::Line(l) => l.deriv().eval(t).to_vec2(),
			PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
			PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
		}
	}

	pub fn at(&self, mut s: f64) -> Option<(kurbo::Point, f64)> {
		if self.total_length < 1e-9 {
			return None;
		}

		if self.is_closed {
			s = s.rem_euclid(self.total_length);
		} else if !(0.0..=self.total_length).contains(&s) {
			return None;
		}

		let idx = self.lengths.partition_point(|&l| l <= s).saturating_sub(1);
		let next_idx = (idx + 1).min(self.lengths.len() - 1);

		let l0 = self.lengths[idx];
		let l1 = self.lengths[next_idx];

		let (seg_idx0, t0) = self.params[idx];
		let (seg_idx1, t1) = self.params[next_idx];

		// Interpolate t within the segment
		let t = if seg_idx0 == seg_idx1 && (l1 - l0) > 1e-9 { t0 + (t1 - t0) * (s - l0) / (l1 - l0) } else { t0 };

		let seg = self.segs.get(seg_idx0)?;
		let point = seg.eval(t);
		let tangent = Self::eval_tangent(*seg, t);

		Some((point, tangent.y.atan2(tangent.x)))
	}

	fn at_or_zero(&self, s: f64) -> (kurbo::Point, f64) {
		self.at(s).unwrap_or((kurbo::Point::ZERO, 0.0))
	}
}

fn extend_along_tangent(point: kurbo::Point, angle: f64, distance: f64) -> kurbo::Point {
	kurbo::Point::new(point.x + distance * angle.cos(), point.y + distance * angle.sin())
}

fn at_with_extension(lut: &ArcLengthLut, s: f64) -> (kurbo::Point, f64) {
	if (0.0..=lut.total_length).contains(&s) {
		return lut.at_or_zero(s);
	}

	if s < 0.0 {
		let (point, angle) = lut.at_or_zero(0.0);
		(extend_along_tangent(point, angle, s), angle)
	} else {
		let (point, angle) = lut.at_or_zero(lut.total_length);
		(extend_along_tangent(point, angle, s - lut.total_length), angle)
	}
}

fn reverse_bezpath(path: BezPath) -> BezPath {
	let mut subpaths = Vec::new();
	let mut current_subpath = Vec::new();

	for el in path.elements() {
		match el {
			PathEl::MoveTo(_) => {
				if !current_subpath.is_empty() {
					subpaths.push(BezPath::from_vec(std::mem::take(&mut current_subpath)));
				}
				current_subpath.push(*el);
			}
			_ => current_subpath.push(*el),
		}
	}
	if !current_subpath.is_empty() {
		subpaths.push(BezPath::from_vec(current_subpath));
	}

	let mut reversed_path = BezPath::new();
	for subpath in subpaths.into_iter().rev() {
		let segs: Vec<_> = subpath.segments().collect();
		if segs.is_empty() {
			if let Some(PathEl::MoveTo(p)) = subpath.elements().first() {
				reversed_path.push(PathEl::MoveTo(*p));
			}
			continue;
		}

		reversed_path.push(PathEl::MoveTo(segs.last().unwrap().end()));
		for seg in segs.iter().rev() {
			match seg {
				PathSeg::Line(l) => reversed_path.push(PathEl::LineTo(l.p0)),
				PathSeg::Quad(q) => reversed_path.push(PathEl::QuadTo(q.p1, q.p0)),
				PathSeg::Cubic(c) => reversed_path.push(PathEl::CurveTo(c.p2, c.p1, c.p0)),
			}
		}

		if subpath.elements().last() == Some(&PathEl::ClosePath) {
			reversed_path.push(PathEl::ClosePath);
		}
	}
	reversed_path
}

fn maybe_reverse_path(path: BezPath, side: TextPathSide) -> BezPath {
	match side {
		TextPathSide::Left => path,
		TextPathSide::Right => reverse_bezpath(path),
	}
}

fn is_glyph_hidden(mid: f64, start_offset: f64, total_length: f64, is_closed: bool, text_anchor: TextAnchor) -> bool {
	if !is_closed {
		return mid < 0.0 || mid > total_length;
	}
	let d = mid - start_offset;
	match text_anchor {
		TextAnchor::Start => d < 0.0 || d > total_length,
		TextAnchor::Middle => d < -total_length / 2.0 || d > total_length / 2.0,
		TextAnchor::End => d < -total_length || d > 0.0,
	}
}

fn resolve_startpoint(abs_offset: f64, total_advance: f64, text_anchor: TextAnchor) -> f64 {
	match text_anchor {
		TextAnchor::Start => abs_offset,
		TextAnchor::Middle => abs_offset - total_advance / 2.0,
		TextAnchor::End => abs_offset - total_advance,
	}
}

pub fn place_text_on_path<Upstream: Default + 'static>(
	text: &str,
	path_table: &Table<Vector<Upstream>>,
	font: &crate::Font,
	font_size: f64,
	character_spacing: f64,
	start_offset: f64,
	start_offset_percent: bool,
	side: TextPathSide,
	text_anchor: TextAnchor,
	font_cache: &crate::FontCache,
) -> Table<Vector<Upstream>> {
	let Some(path_row) = path_table.iter().next() else { return Table::new() };
	let bezpath = path_row.element.stroke_bezpath_iter().find(|path| path.segments().next().is_some());
	let Some(bezpath) = bezpath else { return Table::new() };

	let bezpath = maybe_reverse_path(bezpath, side);
	let lut = ArcLengthLut::build(&bezpath, 100);
	if lut.total_length < 1e-9 {
		return Table::new();
	}

	let typesetting = crate::TypesettingConfig {
		font_size,
		character_spacing,
		..crate::TypesettingConfig::default()
	};

	let layout = crate::TextContext::with_thread_local(|ctx| ctx.layout_text(text, font, font_cache, typesetting));
	let Some(layout) = layout else { return Table::new() };

	let abs_offset = if start_offset_percent { start_offset * lut.total_length } else { start_offset };

	let mut path_builder = crate::path_builder::PathBuilder::new(true, layout.scale() as f64);

	layout.lines().for_each(|line| {
		let line_width = line.metrics().advance as f64;
		let line_start = resolve_startpoint(abs_offset, line_width, text_anchor);

		line.items().for_each(|item| {
			if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
				let mut run_x = glyph_run.offset();
				let run = glyph_run.run();
				let style_skew = run.synthesis().skew().map(|angle| DAffine2::from_cols_array(&[1., 0., -(angle as f64).to_radians().tan(), 1., 0., 0.]));
				let font = run.font();
				let font_size = run.font_size();
				let normalized_coords = run.normalized_coords().iter().map(|coord| skrifa::instance::NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();
				let outlines = ReadFontsRef::from_index(font.data.as_ref(), font.index).unwrap().outline_glyphs();

				glyph_run.glyphs().for_each(|glyph| {
					let glyph_path_pos = line_start + run_x as f64;
					let mid = glyph_path_pos + glyph.advance as f64 / 2.0;
					run_x += glyph.advance;

					if !is_glyph_hidden(mid, abs_offset, lut.total_length, lut.is_closed, text_anchor) {
						let effective_mid = if lut.is_closed { mid.rem_euclid(lut.total_length) } else { mid };
						let (point, angle) = if lut.is_closed { lut.at_or_zero(effective_mid) } else { at_with_extension(&lut, effective_mid) };

						if let Some(glyph_outline) = outlines.get(skrifa::GlyphId::from(glyph.id)) {
							let final_transform = DAffine2::from_translation(DVec2::new(point.x, point.y))
								* DAffine2::from_angle(angle)
								* DAffine2::from_translation(DVec2::new(glyph.x as f64, -glyph.y as f64));
							path_builder.draw_glyph(&glyph_outline, font_size, &normalized_coords, style_skew, final_transform, true);
						}
					}
				});
			}
		});
	});

	path_builder.finalize()
}
