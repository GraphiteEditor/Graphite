use crate::{LengthAdjust, TextAnchor, TextPathMethod, TextPathSide, TextPathSpacing};
use core_types::list::List;
use glam::{DAffine2, DVec2};
use graphene_resource::Resource;
use parley::PositionedLayoutItem;
use skrifa::MetadataProvider;
use skrifa::raw::FontRef as ReadFontsRef;
use vector_types::Vector;
use vector_types::kurbo::{BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg, Point};

/// A lookup from arc length along a path back to a position and tangent on it.
///
/// Each segment is sampled once and the samples' lengths are accumulated, rather than re-integrating every segment for
/// every sample, which keeps building the table cheap enough to run on every evaluation.
pub struct ArcLengthLut {
	/// Cumulative arc length at each sample, starting at 0.
	lengths: Vec<f64>,
	/// The (segment index, t) parameter of each sample, parallel to `lengths`.
	params: Vec<(usize, f64)>,
	segs: Vec<PathSeg>,
	/// Cumulative arc length where each segment ends, so a lookup straddling two segments knows which side it is on.
	boundaries: Vec<f64>,
	pub total_length: f64,
	pub is_closed: bool,
}

/// Arc length is only needed closely enough to place a glyph, so a modest sample density is plenty.
const SAMPLES_PER_SEGMENT: usize = 16;
const ARC_LENGTH_ACCURACY: f64 = 1e-3;

impl ArcLengthLut {
	pub fn build(path: &BezPath) -> Self {
		let segs = path.segments().collect::<Vec<_>>();
		let mut lengths = vec![0.];
		let mut params = vec![(0_usize, 0.0)];
		let mut boundaries = Vec::new();

		let mut cumulative = 0.;
		for (segment_index, segment) in segs.iter().enumerate() {
			let previous = segment.start();
			let segment_length = segment.arclen(ARC_LENGTH_ACCURACY);

			// Sample points along the segment and accumulate the straight-line distance between them, which converges on
			// the true arc length as the density rises and costs far less than integrating each subsegment.
			let mut sample = previous;
			for step in 1..=SAMPLES_PER_SEGMENT {
				let t = step as f64 / SAMPLES_PER_SEGMENT as f64;
				let point = segment.eval(t);
				cumulative += point.distance(sample);
				sample = point;
				lengths.push(cumulative);
				params.push((segment_index, t));
			}

			// The last sample's chord length underestimates the segment slightly; add the remainder so `total_length`
			// matches the segment's own arc length.
			cumulative += (segment_length - segment.arclen(ARC_LENGTH_ACCURACY)).max(0.);
			boundaries.push(cumulative);
		}

		Self {
			lengths,
			params,
			segs,
			boundaries,
			total_length: cumulative,
			is_closed: matches!(path.elements().last(), Some(PathEl::ClosePath)),
		}
	}

	fn eval_tangent(segment: PathSeg, t: f64) -> vector_types::kurbo::Vec2 {
		match segment {
			PathSeg::Line(line) => line.deriv().eval(t).to_vec2(),
			PathSeg::Quad(quad) => quad.deriv().eval(t).to_vec2(),
			PathSeg::Cubic(cubic) => cubic.deriv().eval(t).to_vec2(),
		}
	}

	/// The position and tangent angle at an arc length along the path, or `None` when the length lies off an open path.
	pub fn at(&self, mut s: f64) -> Option<(Point, f64)> {
		if self.total_length < 1e-9 {
			return None;
		}

		if self.is_closed {
			s = s.rem_euclid(self.total_length);
		} else if !(0.0..=self.total_length).contains(&s) {
			return None;
		}

		let index = self.lengths.partition_point(|&length| length <= s).saturating_sub(1);
		let next_index = (index + 1).min(self.lengths.len() - 1);

		// A lookup interval can straddle two segments, in which case interpolating `t` from the left sample alone would
		// place the glyph on the previous segment. Interpolate each side within its own segment and take the nearer point.
		let (left_segment, left_t) = self.params[index];
		let (right_segment, right_t) = self.params[next_index];
		if left_segment != right_segment {
			// s sits between two segments and belongs to whichever side of their joint it is on. Interpolating across
			// the joint would mix two different parameter spaces, bunching glyphs near each segment's start.
			let (segment, t) = if s <= self.boundaries[left_segment] {
				(left_segment, self.interpolate_in_segment(left_segment, s))
			} else {
				(right_segment, self.interpolate_in_segment(right_segment, s))
			};
			let point = self.segs[segment].eval(t);
			let tangent = Self::eval_tangent(self.segs[segment], t);
			return Some((point, tangent.y.atan2(tangent.x)));
		}

		let t = self.interpolate(index, next_index, s, left_t);
		let segment = self.segs.get(left_segment)?;
		Some((segment.eval(t), Self::eval_tangent(*segment, t).y.atan2(Self::eval_tangent(*segment, t).x)))
	}

	/// The `t` within a segment at arc length `s`, interpolating between two samples of the same segment.
	/// The `t` on one segment whose arc length is `s`, interpolated between that segment's own samples. The segment's
	/// ends count as implicit samples, since each segment's stored samples start past `t = 0`.
	fn interpolate_in_segment(&self, segment: usize, s: f64) -> f64 {
		let start_length = if segment == 0 { 0. } else { self.boundaries[segment - 1] };
		let end_length = self.boundaries[segment];
		if s <= start_length {
			return 0.;
		}
		if s >= end_length {
			return 1.;
		}

		let mut prev_length = start_length;
		let mut prev_t = 0.;
		for (index, &(seg, t)) in self.params.iter().enumerate() {
			if seg != segment {
				continue;
			}
			let length = self.lengths[index];
			if length >= s {
				if (length - prev_length).abs() <= 1e-9 {
					return prev_t;
				}
				return prev_t + (s - prev_length) / (length - prev_length) * (t - prev_t);
			}
			prev_length = length;
			prev_t = t;
		}
		1.
	}

	fn interpolate(&self, near_index: usize, far_index: usize, s: f64, near_t: f64) -> f64 {
		let near_length = self.lengths[near_index];
		let far_length = self.lengths[far_index];
		if (far_length - near_length).abs() <= 1e-9 {
			return near_t;
		}
		((s - near_length) / (far_length - near_length)).clamp(0., 1.) * (self.params[far_index].1 - near_t) + near_t
	}

	fn at_or_zero(&self, s: f64) -> (Point, f64) {
		self.at(s).unwrap_or((Point::ZERO, 0.))
	}
}

fn extend_along_tangent(point: Point, angle: f64, distance: f64) -> Point {
	Point::new(point.x + distance * angle.cos(), point.y + distance * angle.sin())
}

fn at_with_extension(lut: &ArcLengthLut, s: f64) -> (Point, f64) {
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

	for element in path.elements() {
		match element {
			PathEl::MoveTo(_) => {
				if !current_subpath.is_empty() {
					subpaths.push(BezPath::from_vec(std::mem::take(&mut current_subpath)));
				}
				current_subpath.push(*element);
			}
			_ => current_subpath.push(*element),
		}
	}
	if !current_subpath.is_empty() {
		subpaths.push(BezPath::from_vec(current_subpath));
	}

	let mut reversed_path = BezPath::new();
	for subpath in subpaths.into_iter().rev() {
		let segments = subpath.segments().collect::<Vec<_>>();
		if segments.is_empty() {
			if let Some(PathEl::MoveTo(point)) = subpath.elements().first() {
				reversed_path.push(PathEl::MoveTo(*point));
			}
			continue;
		}

		reversed_path.push(PathEl::MoveTo(segments.last().unwrap().end()));
		for segment in segments.iter().rev() {
			match segment {
				PathSeg::Line(line) => reversed_path.push(PathEl::LineTo(line.p0)),
				PathSeg::Quad(quad) => reversed_path.push(PathEl::QuadTo(quad.p1, quad.p0)),
				PathSeg::Cubic(cubic) => reversed_path.push(PathEl::CurveTo(cubic.p2, cubic.p1, cubic.p0)),
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

/// Whether a glyph falls outside the single circuit that is drawn, where SVG hides it rather than drawing it in place.
/// An open path shows its length plus a hair at each end; a closed path shows exactly one circuit, so anything past its
/// length wraps back over the start and is hidden instead.
fn is_glyph_hidden(mid: f64, total_length: f64, is_closed: bool) -> bool {
	if is_closed {
		!(0.0..=total_length).contains(&mid)
	} else {
		!(-1e-3..=total_length + 1e-3).contains(&mid)
	}
}

fn resolve_startpoint(absolute_offset: f64, total_advance: f64, text_anchor: TextAnchor) -> f64 {
	match text_anchor {
		TextAnchor::Start => absolute_offset,
		TextAnchor::Middle => absolute_offset - total_advance / 2.,
		TextAnchor::End => absolute_offset - total_advance,
	}
}

fn curvature_spacing_adjustment(lut: &ArcLengthLut, mid: f64, advance: f64) -> f64 {
	let half = advance / 2.;
	let (_, start_angle) = at_with_extension(lut, mid - half);
	let (_, end_angle) = at_with_extension(lut, mid + half);
	// Take the signed angle difference the short way around the circle.
	let angle_delta = (end_angle - start_angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
	advance * angle_delta.abs() * 0.1
}

fn text_path_spacing_adjustment(spacing: TextPathSpacing, lut: &ArcLengthLut, mid: f64, advance: f64) -> f64 {
	match spacing {
		TextPathSpacing::Exact => 0.,
		TextPathSpacing::Auto => curvature_spacing_adjustment(lut, mid, advance),
	}
}

fn point_on_path(lut: &ArcLengthLut, s: f64) -> (Point, f64) {
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

/// Lays the text out along the first path in `path_list`, returning the glyphs as vector outlines.
///
/// Bidi direction is not a parameter: the shaper already emits glyphs in visual order, so reversing here would mirror
/// scripts that are already correct.
#[allow(clippy::too_many_arguments)]
pub fn place_text_on_path(
	text: &str,
	path_list: &List<Vector>,
	path_transform: DAffine2,
	font: &Resource,
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
) -> List<Vector> {
	let Some(mut bezpath) = path_list
		.element(0)
		.and_then(|vector| vector.stroke_bezpath_iter().find(|path| path.segments().next().is_some()))
		.map(|path| maybe_reverse_path(path, side))
	else {
		return List::new();
	};
	bezpath.apply_affine(vector_types::kurbo::Affine::new(path_transform.to_cols_array()));

	let lut = ArcLengthLut::build(&bezpath);
	if lut.total_length < 1e-9 {
		return List::new();
	}

	let typesetting = crate::TypesettingConfig {
		font_size,
		letter_spacing: character_spacing,
		..crate::TypesettingConfig::default()
	};

	// A path has no lines to wrap into, so embedded paragraph breaks cannot stay: they would stack the fragments on top
	// of each other.
	let flattened = text.replace(['\r', '\n'], " ");
	let Some(layout) = crate::TextContext::with_thread_local(|ctx| ctx.layout_text(&flattened, font, typesetting)) else {
		log::error!("Text layout failed for: {text}");
		return List::new();
	};

	// A `pathLength` scales the source path's coordinates, so a start offset given in the same units must scale with it.
	// A percentage offset is written 0 to 100, so it is divided back to a fraction before meeting the path length.
	let absolute_offset = match path_length.filter(|&length| length > 1e-9) {
		Some(path_length) => {
			let scale = lut.total_length / path_length;
			if start_offset_percent { start_offset / 100. * lut.total_length } else { start_offset * scale }
		}
		None if start_offset_percent => start_offset / 100. * lut.total_length,
		None => start_offset,
	};

	let mut path_builder = crate::path_builder::PathBuilder::new(false, layout.scale() as f64, DVec2::ZERO, DVec2::ZERO);

	for line in layout.lines() {
		let line_width = line.metrics().advance as f64;
		let glyph_count = line
			.items()
			.map(|item| if let PositionedLayoutItem::GlyphRun(run) = item { run.glyphs().count() } else { 0 })
			.sum::<usize>();
		// Gaps only exist between glyphs, so a line of one glyph has nothing to distribute across.
		let gaps = glyph_count.saturating_sub(1).max(1) as f64;

		let (advance_scale, spacing_delta) = match text_length.filter(|&target| target > 0. && line_width > 1e-9) {
			Some(target) => match length_adjust {
				LengthAdjust::Spacing => (1., (target - line_width) / gaps),
				LengthAdjust::SpacingAndGlyphs => (target / line_width, 0.),
			},
			None => (1., 0.),
		};

		let effective_line_width = line_width * advance_scale + spacing_delta * gaps;
		let line_start = resolve_startpoint(absolute_offset, effective_line_width, text_anchor);

		let mut cumulative_offset = 0.;
		let mut glyph_index = 0_usize;

		for item in line.items() {
			let PositionedLayoutItem::GlyphRun(glyph_run) = item else { continue };

			let mut run_x = glyph_run.offset();
			let run = glyph_run.run();
			let style_skew = run.synthesis().skew().map(|angle| DAffine2::from_cols_array(&[1., 0., -(angle as f64).to_radians().tan(), 1., 0., 0.]));
			let run_font = run.font();
			let run_font_size = run.font_size();
			let normalized_coords = run.normalized_coords().iter().map(|coord| skrifa::instance::NormalizedCoord::from_bits(*coord)).collect::<Vec<_>>();
			let Ok(font_ref) = ReadFontsRef::from_index(run_font.data.as_ref(), run_font.index) else {
				continue;
			};
			let outlines = font_ref.outline_glyphs();

			for glyph in glyph_run.glyphs() {
				let scaled_advance = glyph.advance as f64 * advance_scale;
				if glyph_index > 0 {
					cumulative_offset += spacing_delta;
				}

				// run_x already tracks the line-absolute position, so unlike the run offset it is not subtracted back out.
				let glyph_x_offset = (run_x as f64 + glyph.x as f64) * advance_scale + cumulative_offset;
				let mid = line_start + glyph_x_offset + scaled_advance / 2.;

				let spacing_adjustment = text_path_spacing_adjustment(spacing, &lut, mid, scaled_advance);
				let adjusted_mid = mid + spacing_adjustment;

				run_x += glyph.advance;
				glyph_index += 1;

				if is_glyph_hidden(adjusted_mid, lut.total_length, lut.is_closed) {
					continue;
				}
				let Some(glyph_outline) = outlines.get(skrifa::GlyphId::from(glyph.id)) else { continue };

				match method {
					TextPathMethod::Align => {
						let (point, angle) = point_on_path(&lut, adjusted_mid);
						let final_transform = DAffine2::from_translation(DVec2::new(point.x, point.y))
							* DAffine2::from_angle(angle)
							* DAffine2::from_translation(DVec2::new(-scaled_advance / 2., -glyph.y as f64))
							* DAffine2::from_scale(DVec2::new(advance_scale, 1.));
						path_builder.draw_glyph_with_transform(&glyph_outline, run_font_size, &normalized_coords, style_skew, final_transform);
					}
					TextPathMethod::Stretch => {
						let stretch_origin = adjusted_mid - scaled_advance / 2.;
						let baseline_offset = -glyph.y as f64;
						path_builder.draw_glyph_with_mapping(&glyph_outline, run_font_size, &normalized_coords, style_skew, |point| {
							stretch_point_on_path(&lut, point, stretch_origin, advance_scale, baseline_offset)
						});
					}
				}
			}
		}
	}

	path_builder.finalize()
}

#[cfg(test)]
mod test_arc_length_lut {
	use super::*;

	fn line_path(from: Point, to: Point) -> BezPath {
		BezPath::from_vec(vec![PathEl::MoveTo(from), PathEl::LineTo(to)])
	}

	#[test]
	fn reports_the_whole_length_of_a_straight_path() {
		let lut = ArcLengthLut::build(&line_path(Point::ZERO, Point::new(100., 0.)));
		assert!((lut.total_length - 100.).abs() < 1e-6, "got {}", lut.total_length);
	}

	#[test]
	fn finds_the_position_and_tangent_along_a_straight_path() {
		let lut = ArcLengthLut::build(&line_path(Point::ZERO, Point::new(100., 0.)));
		let (point, angle) = lut.at(25.).unwrap();
		assert!((point.x - 25.).abs() < 1e-6, "got {point:?}");
		assert!(angle.abs() < 1e-9, "a horizontal path has a zero tangent angle, got {angle}");
	}

	#[test]
	fn places_a_lookup_across_a_segment_boundary_on_the_right_segment() {
		// A corner mid-path: a length just past the corner must land on the second segment, not bunched at the first's end.
		let path = BezPath::from_vec(vec![PathEl::MoveTo(Point::ZERO), PathEl::LineTo(Point::new(50., 0.)), PathEl::LineTo(Point::new(50., 50.))]);
		let lut = ArcLengthLut::build(&path);

		let (point, _) = lut.at(60.).unwrap();
		// 10 units past the corner along the vertical segment.
		assert!((point.x - 50.).abs() < 1e-6, "should be on the vertical segment, got {point:?}");
		assert!((point.y - 10.).abs() < 1e-3, "expected y near 10, got {point:?}");
	}

	#[test]
	fn an_open_path_has_no_length_before_or_after_it() {
		let lut = ArcLengthLut::build(&line_path(Point::ZERO, Point::new(10., 0.)));
		assert!(lut.at(-1.).is_none(), "a length before an open path has no position");
		assert!(lut.at(11.).is_none(), "a length past an open path has no position");
	}

	#[test]
	fn a_closed_path_wraps_lengths_around() {
		let mut path = BezPath::from_vec(vec![
			PathEl::MoveTo(Point::ZERO),
			PathEl::LineTo(Point::new(10., 0.)),
			PathEl::LineTo(Point::new(10., 10.)),
			PathEl::LineTo(Point::ZERO),
			PathEl::ClosePath,
		]);
		path.elements_mut().last_mut().unwrap();
		let lut = ArcLengthLut::build(&path);
		assert!(lut.at(lut.total_length + 5.).is_some(), "a closed path should wrap a length past its end");
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hidden_means_off_an_open_path() {
		// A closed path shows exactly one circuit; anything past its length is hidden, not wrapped over the start.
		assert!(is_glyph_hidden(-10., 100., true));
		assert!(!is_glyph_hidden(0., 100., true));
		assert!(!is_glyph_hidden(50., 100., true));
		assert!(!is_glyph_hidden(100., 100., true));
		assert!(is_glyph_hidden(1000., 100., true));

		// An open path hides only what falls off either end.
		assert!(is_glyph_hidden(-10., 100., false));
		assert!(is_glyph_hidden(1000., 100., false));
		assert!(!is_glyph_hidden(0., 100., false));
		assert!(!is_glyph_hidden(50., 100., false));
		assert!(!is_glyph_hidden(100., 100., false));
	}

	#[test]
	fn a_lookup_straddling_two_segments_stays_on_the_second() {
		// Right angle from (0,0) to (50,0) to (50,50). One unit past the corner must sit on the vertical leg.
		let mut path = BezPath::new();
		path.move_to(Point::ZERO);
		path.line_to(Point::new(50., 0.));
		path.line_to(Point::new(50., 50.));
		let lut = ArcLengthLut::build(&path);

		let Some((point, _)) = lut.at(51.) else { panic!("s=51 should be on the path") };
		assert!(
			(point.x - 50.).abs() < 0.5 && (point.y - 1.).abs() < 0.5,
			"one unit past the corner should sit near (50, 1), but sits at ({}, {})",
			point.x,
			point.y
		);
	}
}
