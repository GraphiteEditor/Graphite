use core_types::table::Table;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use kurbo::{BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg};
use parley::PositionedLayoutItem;
use skrifa::MetadataProvider;
use skrifa::raw::FontRef as ReadFontsRef;
use std::sync::Arc;
use vector_types::{TextOnPathMetadata, Vector};

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash, serde::Serialize, serde::Deserialize, DynAny, node_macro::ChoiceType)]
pub enum TextPathMethod {
	#[default]
	Align,
	Stretch,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash, serde::Serialize, serde::Deserialize, DynAny, node_macro::ChoiceType)]
pub enum TextPathSpacing {
	#[default]
	Exact,
	Auto,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Hash, serde::Serialize, serde::Deserialize, DynAny, node_macro::ChoiceType)]
pub enum LengthAdjust {
	#[default]
	Spacing,
	SpacingAndGlyphs,
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
		let samples_per_segment = samples_per_segment.max(1);
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

fn is_glyph_hidden(mid: f64, _start_offset: f64, total_length: f64, is_closed: bool, _text_anchor: TextAnchor, _rtl: bool) -> bool {
	if is_closed {
		return false;
	}
	mid < -1e-3 || mid > total_length + 1e-3
}

fn resolve_startpoint(abs_offset: f64, total_advance: f64, text_anchor: TextAnchor, rtl: bool) -> f64 {
	if !rtl {
		match text_anchor {
			TextAnchor::Start => abs_offset,
			TextAnchor::Middle => abs_offset - total_advance / 2.0,
			TextAnchor::End => abs_offset - total_advance,
		}
	} else {
		match text_anchor {
			TextAnchor::Start => abs_offset,
			TextAnchor::Middle => abs_offset + total_advance / 2.0,
			TextAnchor::End => abs_offset + total_advance,
		}
	}
}

fn curvature_spacing_adjustment(lut: &ArcLengthLut, mid: f64, advance: f64) -> f64 {
	let half = advance / 2.0;
	let (_, a0) = at_with_extension(lut, mid - half);
	let (_, a1) = at_with_extension(lut, mid + half);
	let angle_delta = (a1 - a0 + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
	advance * angle_delta.abs() * 0.1
}

fn text_path_spacing_adjustment(spacing: TextPathSpacing, lut: &ArcLengthLut, mid: f64, advance: f64) -> f64 {
	match spacing {
		TextPathSpacing::Exact => 0.0,
		TextPathSpacing::Auto => curvature_spacing_adjustment(lut, mid, advance),
	}
}

fn point_on_path(lut: &ArcLengthLut, s: f64) -> (kurbo::Point, f64) {
	if lut.is_closed {
		lut.at_or_zero(s.rem_euclid(lut.total_length))
	} else {
		at_with_extension(lut, s)
	}
}

fn stretch_point_on_path(lut: &ArcLengthLut, point: DVec2, origin: f64, advance_scale: f64, baseline_offset: f64) -> DVec2 {
	let (path_point, angle) = point_on_path(lut, origin + point.x * advance_scale);
	let normal = DVec2::new(-angle.sin(), angle.cos());
	DVec2::new(path_point.x, path_point.y) + normal * (point.y + baseline_offset)
}

#[allow(clippy::too_many_arguments)]
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
	method: TextPathMethod,
	spacing: TextPathSpacing,
	text_length: Option<f64>,
	length_adjust: LengthAdjust,
	path_length: Option<f64>,
	rtl: bool,
	font_cache: &crate::FontCache,
) -> Table<Vector<Upstream>> {
	let Some(original_bezpath) = path_table.iter().next().and_then(|row| row.element.stroke_bezpath_iter().find(|p| p.segments().next().is_some())) else {
		return Table::new();
	};
	let path_d_for_export = original_bezpath.to_svg();

	let bezpath = maybe_reverse_path(original_bezpath, side);
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
	let Some(layout) = layout else {
		log::error!("Text layout failed for: {}", text);
		return Table::new();
	};

	log::info!("Placing text on path: {} (length: {})", text, lut.total_length);

	let abs_offset = if let Some(pl) = path_length.filter(|&l| l > 1e-9) {
		let scale = lut.total_length / pl;
		let offset = if start_offset_percent { start_offset * lut.total_length } else { start_offset * scale };
		if rtl { lut.total_length - offset } else { offset }
	} else if start_offset_percent {
		let offset = start_offset * lut.total_length;
		if rtl { lut.total_length - offset } else { offset }
	} else if rtl {
		lut.total_length - start_offset
	} else {
		start_offset
	};

	let mut path_builder = crate::path_builder::PathBuilder::new(true, layout.scale() as f64);

	layout.lines().for_each(|line| {
		let line_width = line.metrics().advance as f64;

		let glyph_count: usize = line.items().map(|item| if let PositionedLayoutItem::GlyphRun(gr) = item { gr.glyphs().count() } else { 0 }).sum();

		let (advance_scale, spacing_delta) = if let Some(target) = text_length.filter(|&t| t > 0.0 && line_width > 1e-9) {
			match length_adjust {
				LengthAdjust::Spacing => (1.0, (target - line_width) / glyph_count.saturating_sub(1).max(1) as f64),
				LengthAdjust::SpacingAndGlyphs => (target / line_width, 0.0),
			}
		} else {
			(1.0, 0.0)
		};

		let effective_line_width = line_width * advance_scale + spacing_delta * glyph_count.saturating_sub(1) as f64;
		let line_start = resolve_startpoint(abs_offset, effective_line_width, text_anchor, rtl);

		let mut cumulative_offset = 0.0_f64;
		let mut glyph_index = 0_usize;

		line.items().for_each(|item| {
			if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
				let mut run_x = glyph_run.offset();
				let run = glyph_run.run();
				let style_skew = run.synthesis().skew().map(|angle| DAffine2::from_cols_array(&[1., 0., -(angle as f64).to_radians().tan(), 1., 0., 0.]));
				let font = run.font();
				let font_size = run.font_size();
				let normalized_coords = run.normalized_coords().iter().map(|coord| skrifa::instance::NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();
				let Ok(font_ref) = ReadFontsRef::from_index(font.data.as_ref(), font.index) else { return };
				let outlines = font_ref.outline_glyphs();

				glyph_run.glyphs().for_each(|glyph| {
					let scaled_advance = glyph.advance as f64 * advance_scale;
					cumulative_offset += if glyph_index > 0 { spacing_delta } else { 0.0 };
					
					let glyph_x_offset = (run_x as f64 - glyph_run.offset() as f64 + glyph.x as f64) * advance_scale + cumulative_offset;
					let mid = if rtl { line_start - glyph_x_offset - scaled_advance / 2.0 } else { line_start + glyph_x_offset + scaled_advance / 2.0 };
					
					let spacing_adj = text_path_spacing_adjustment(spacing, &lut, mid, scaled_advance);
					let adjusted_mid = if rtl { mid - spacing_adj } else { mid + spacing_adj };

					run_x += glyph.advance;
					glyph_index += 1;

					if !is_glyph_hidden(adjusted_mid, abs_offset, lut.total_length, lut.is_closed, text_anchor, rtl) {
						if let Some(glyph_outline) = outlines.get(skrifa::GlyphId::from(glyph.id)) {
							match method {
								TextPathMethod::Align => {
									let (point, angle) = point_on_path(&lut, adjusted_mid);
									let final_transform = DAffine2::from_translation(DVec2::new(point.x, point.y))
										* DAffine2::from_angle(angle) * DAffine2::from_translation(DVec2::new(-scaled_advance / 2.0, -glyph.y as f64))
										* DAffine2::from_scale(DVec2::new(advance_scale, 1.0));
									path_builder.draw_glyph(&glyph_outline, font_size, &normalized_coords, style_skew, final_transform, true);
								}
								TextPathMethod::Stretch => {
									let stretch_origin = adjusted_mid - scaled_advance / 2.0;
									let baseline_offset = -glyph.y as f64;
									path_builder.draw_glyph_with_mapping(&glyph_outline, font_size, &normalized_coords, style_skew, |point| {
										stretch_point_on_path(&lut, point, stretch_origin, advance_scale, baseline_offset)
									});
								}
							}
						}
					}
				});
			}
		});
	});

	let mut result = path_builder.finalize();

	// Attach text-on-path metadata so SVG export can emit <text><textPath> instead of raw outlines
	let metadata = Arc::new(TextOnPathMetadata {
		text: text.to_string(),
		font_family: font.font_family.clone(),
		font_style: font.font_style.clone(),
		font_size,
		path_d: path_d_for_export,
		start_offset,
		start_offset_percent,
		text_anchor: match text_anchor {
			TextAnchor::Start => "start",
			TextAnchor::Middle => "middle",
			TextAnchor::End => "end",
		}
		.to_string(),
		side: match side {
			TextPathSide::Left => "left",
			TextPathSide::Right => "right",
		}
		.to_string(),
		method: match method {
			TextPathMethod::Align => "align",
			TextPathMethod::Stretch => "stretch",
		}
		.to_string(),
		spacing: match spacing {
			TextPathSpacing::Exact => "exact",
			TextPathSpacing::Auto => "auto",
		}
		.to_string(),
		text_length,
		length_adjust: match length_adjust {
			LengthAdjust::Spacing => "spacing",
			LengthAdjust::SpacingAndGlyphs => "spacingAndGlyphs",
		}
		.to_string(),
		path_length,
		rtl,
	});
	for row in result.iter_mut() {
		row.element.text_on_path_metadata = Some(Arc::clone(&metadata));
	}

	result
}
