use crate::{LengthAdjust, TextAnchor, TextPathMethod, TextPathSide};
use core_types::list::{Item, List};
use glam::{DAffine2, DVec2};
use graphene_resource::Resource;
use parley::PositionedLayoutItem;
use skrifa::MetadataProvider;
use skrifa::raw::FontRef as ReadFontsRef;
use vector_types::Vector;
use vector_types::kurbo::{BezPath, DEFAULT_ACCURACY, ParamCurve, PathEl, PathSeg, Point, Shape};
use vector_types::vector::algorithms::bezpath_algorithms::{TValue, evaluate_bezpath, tangent_on_bezpath};

/// A lookup from arc length along a path back to a position and tangent angle on it.
///
/// Segment lengths are measured once up front and shared with the evaluation helpers, so placing each glyph
/// walks the path without re-integrating it.
pub struct ArcLengthLut<'a> {
	bezpath: &'a BezPath,
	segment_lengths: Vec<f64>,
	pub total_length: f64,
	pub is_closed: bool,
}

impl<'a> ArcLengthLut<'a> {
	pub fn build(path: &'a BezPath) -> Self {
		let segment_lengths = path.segments().map(|segment| segment.perimeter(DEFAULT_ACCURACY)).collect::<Vec<_>>();
		Self {
			bezpath: path,
			total_length: segment_lengths.iter().sum(),
			segment_lengths,
			is_closed: matches!(path.elements().last(), Some(PathEl::ClosePath)),
		}
	}

	/// The position and tangent angle at an arc length along the path, or `None` when the length lies off an open path.
	pub fn at(&self, mut s: f64) -> Option<(Point, f64)> {
		if self.total_length < 1e-9 {
			return None;
		}

		if self.is_closed {
			s = s.rem_euclid(self.total_length);
		} else if !(0. ..=self.total_length).contains(&s) {
			return None;
		}

		let fraction = (s / self.total_length).clamp(0., 1.);
		let point = evaluate_bezpath(self.bezpath, TValue::Euclidean(fraction), Some(&self.segment_lengths));
		let tangent = tangent_on_bezpath(self.bezpath, TValue::Euclidean(fraction), Some(&self.segment_lengths));
		Some((point, tangent.y.atan2(tangent.x)))
	}

	fn at_or_zero(&self, s: f64) -> (Point, f64) {
		self.at(s).unwrap_or((Point::ZERO, 0.))
	}
}

fn extend_along_tangent(point: Point, angle: f64, distance: f64) -> Point {
	Point::new(point.x + distance * angle.cos(), point.y + distance * angle.sin())
}

fn at_with_extension(lut: &ArcLengthLut, s: f64) -> (Point, f64) {
	if (0. ..=lut.total_length).contains(&s) {
		return lut.at_or_zero(s);
	}

	if s < 0. {
		let (point, angle) = lut.at_or_zero(0.);
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
		!(0. ..=total_length).contains(&mid)
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

/// How text sits on its path: which side, where it starts, and how it fits a target length.
#[derive(Debug, Clone, Copy)]
pub struct TextPathConfig {
	pub side: TextPathSide,
	pub text_anchor: TextAnchor,
	pub start_offset: f64,
	pub method: TextPathMethod,
	pub text_length: Option<f64>,
	pub length_adjust: LengthAdjust,
}

/// Lays the text out along the first path in `path_list`, returning the glyphs as vector outlines.
///
/// Bidi direction is not a parameter: the shaper already emits glyphs in visual order, so reversing here would mirror
/// scripts that are already correct.
pub fn place_text_on_path(
	text: &str,
	path_list: &List<Vector>,
	path_transform: DAffine2,
	font: &Resource,
	font_size: f64,
	character_spacing: f64,
	letter_tilt: f64,
	config: TextPathConfig,
) -> Item<Vector> {
	let TextPathConfig {
		side,
		text_anchor,
		start_offset,
		method,
		text_length,
		length_adjust,
	} = config;
	let Some(mut bezpath) = path_list
		.element(0)
		.and_then(|vector| vector.stroke_bezpath_iter().find(|path| path.segments().next().is_some()))
		.map(|path| maybe_reverse_path(path, side))
	else {
		return Item::new_from_element(Vector::default());
	};
	bezpath.apply_affine(vector_types::kurbo::Affine::new(path_transform.to_cols_array()));

	let lut = ArcLengthLut::build(&bezpath);
	if lut.total_length < 1e-9 {
		return Item::new_from_element(Vector::default());
	}

	let typesetting = crate::TypesettingConfig {
		font_size,
		letter_spacing: character_spacing,
		letter_tilt,
		..crate::TypesettingConfig::default()
	};

	// A path has no lines to wrap into, so embedded paragraph breaks cannot stay: they would stack the fragments on top
	// of each other.
	let flattened = text.replace(['\r', '\n'], " ");
	let Some(layout) = crate::TextContext::with_thread_local(|ctx| ctx.layout_text(&flattened, font, typesetting)) else {
		log::error!("Text layout failed for: {text}");
		return Item::new_from_element(Vector::default());
	};

	// The offset is written 0 to 100 like the SVG percentage it mirrors, so it is divided back to a fraction before
	// meeting the path length. Negative values and values past 100% are allowed.
	let absolute_offset = start_offset / 100. * lut.total_length;

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

				run_x += glyph.advance;
				glyph_index += 1;

				if is_glyph_hidden(mid, lut.total_length, lut.is_closed) {
					continue;
				}
				let Some(glyph_outline) = outlines.get(skrifa::GlyphId::from(glyph.id)) else { continue };

				// The user's faux-italic slant, sheared in glyph space before the path placement orients it.
				let tilt = DAffine2::from_cols_array(&[1., 0., -letter_tilt.to_radians().tan(), 1., 0., 0.]);
				match method {
					TextPathMethod::Align => {
						let (point, angle) = point_on_path(&lut, mid);
						let final_transform = DAffine2::from_translation(DVec2::new(point.x, point.y))
							* DAffine2::from_angle(angle)
							* tilt
							* DAffine2::from_translation(DVec2::new(-scaled_advance / 2., -glyph.y as f64))
							* DAffine2::from_scale(DVec2::new(advance_scale, 1.));
						path_builder.draw_glyph_with_transform(&glyph_outline, run_font_size, &normalized_coords, style_skew, final_transform);
					}
					TextPathMethod::Stretch => {
						let stretch_origin = mid - scaled_advance / 2.;
						let baseline_offset = -glyph.y as f64;
						path_builder.draw_glyph_with_mapping(&glyph_outline, run_font_size, &normalized_coords, style_skew, |point| {
							stretch_point_on_path(&lut, tilt.transform_point2(point), stretch_origin, advance_scale, baseline_offset)
						});
					}
				}
			}
		}
	}

	path_builder.finalize_without_text_frame().into_iter().next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::FALLBACK_FONT_RESOURCE;

	fn line_path(from: Point, to: Point) -> BezPath {
		BezPath::from_vec(vec![PathEl::MoveTo(from), PathEl::LineTo(to)])
	}

	fn path_list(path: BezPath) -> List<Vector> {
		List::new_from_element(Vector::from_bezpath(path))
	}

	fn place_on_line(text: &str, end: Point, text_anchor: TextAnchor) -> Item<Vector> {
		place_on_line_at_offset(text, end, text_anchor, 0.)
	}

	fn place_on_line_at_offset(text: &str, end: Point, text_anchor: TextAnchor, start_offset: f64) -> Item<Vector> {
		place_text_on_path(
			text,
			&path_list(line_path(Point::ZERO, end)),
			DAffine2::IDENTITY,
			&FALLBACK_FONT_RESOURCE,
			24.,
			0.,
			0.,
			TextPathConfig {
				side: TextPathSide::Left,
				text_anchor,
				start_offset,
				method: TextPathMethod::Align,
				text_length: None,
				length_adjust: LengthAdjust::Spacing,
			},
		)
	}

	fn placed_bounds(item: &Item<Vector>) -> [DVec2; 2] {
		item.element().bounding_box().expect("placed text should have bounds")
	}

	#[test]
	fn test_reports_the_whole_length_of_a_straight_path() {
		let path = line_path(Point::ZERO, Point::new(100., 0.));
		let lut = ArcLengthLut::build(&path);
		assert!((lut.total_length - 100.).abs() < 1e-6, "got {}", lut.total_length);
	}

	#[test]
	fn test_finds_the_position_and_tangent_along_a_straight_path() {
		let path = line_path(Point::ZERO, Point::new(100., 0.));
		let lut = ArcLengthLut::build(&path);
		let (point, angle) = lut.at(25.).unwrap();
		assert!((point.x - 25.).abs() < 1e-6, "got {point:?}");
		assert!(angle.abs() < 1e-9, "a horizontal path has a zero tangent angle, got {angle}");
	}

	#[test]
	fn test_lookup_across_a_segment_boundary_stays_on_the_second_segment() {
		// Right angle from (0,0) to (50,0) to (50,50). One unit past the corner must sit on the vertical leg.
		let path = BezPath::from_vec(vec![PathEl::MoveTo(Point::ZERO), PathEl::LineTo(Point::new(50., 0.)), PathEl::LineTo(Point::new(50., 50.))]);
		let lut = ArcLengthLut::build(&path);

		let Some((point, _)) = lut.at(51.) else { panic!("s=51 should be on the path") };
		assert!(
			(point.x - 50.).abs() < 0.5 && (point.y - 1.).abs() < 0.5,
			"one unit past the corner should sit near (50, 1), but sits at ({}, {})",
			point.x,
			point.y
		);
	}

	#[test]
	fn test_open_path_has_no_length_before_or_after_it() {
		let path = line_path(Point::ZERO, Point::new(10., 0.));
		let lut = ArcLengthLut::build(&path);
		assert!(lut.at(-1.).is_none(), "a length before an open path has no position");
		assert!(lut.at(11.).is_none(), "a length past an open path has no position");
	}

	#[test]
	fn test_closed_path_wraps_lengths_around() {
		let path = BezPath::from_vec(vec![
			PathEl::MoveTo(Point::ZERO),
			PathEl::LineTo(Point::new(10., 0.)),
			PathEl::LineTo(Point::new(10., 10.)),
			PathEl::LineTo(Point::ZERO),
			PathEl::ClosePath,
		]);
		let lut = ArcLengthLut::build(&path);
		assert!(lut.at(lut.total_length + 5.).is_some(), "a closed path should wrap a length past its end");
	}

	#[test]
	fn test_hidden_means_off_the_path() {
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
	fn test_text_on_a_straight_line_starts_at_the_path_start() {
		let [min, max] = placed_bounds(&place_on_line("Hi", Point::new(200., 0.), TextAnchor::Start));
		assert!(min.x >= -1. && min.x <= 5., "the first glyph should start at the path start, got {min:?}");
		assert!(max.x > min.x && max.y > min.y, "placed text should have area, got [{min:?}, {max:?}]");
		assert!(min.y >= -30. && max.y <= 5., "glyphs should sit on the line, got [{min:?}, {max:?}]");
	}

	#[test]
	fn test_middle_anchor_centers_text_on_the_start_offset() {
		let start = placed_bounds(&place_on_line("Hi", Point::new(200., 0.), TextAnchor::Start));
		let middle = placed_bounds(&place_on_line_at_offset("Hi", Point::new(200., 0.), TextAnchor::Middle, 50.));

		assert!(middle[0].x > start[0].x, "centered text should start further along, got {middle:?} vs {start:?}");
		let center = (middle[0].x + middle[1].x) / 2.;
		assert!((center - 100.).abs() < 5., "text centered on the 50% offset should sit around the path midpoint, got {center}");
	}

	#[test]
	fn test_overflow_past_one_circuit_stays_hidden() {
		let square = BezPath::from_vec(vec![
			PathEl::MoveTo(Point::ZERO),
			PathEl::LineTo(Point::new(10., 0.)),
			PathEl::LineTo(Point::new(10., 10.)),
			PathEl::LineTo(Point::new(0., 10.)),
			PathEl::ClosePath,
		]);
		let item = place_text_on_path(
			"Hello world, this overflows the little square several times over",
			&path_list(square),
			DAffine2::IDENTITY,
			&FALLBACK_FONT_RESOURCE,
			24.,
			0.,
			0.,
			TextPathConfig {
				side: TextPathSide::Left,
				text_anchor: TextAnchor::Start,
				start_offset: 0.,
				method: TextPathMethod::Align,
				text_length: None,
				length_adjust: LengthAdjust::Spacing,
			},
		);
		let [min, max] = placed_bounds(&item);
		assert!(
			min.x >= -30. && min.y >= -30. && max.x <= 40. && max.y <= 40.,
			"text past one circuit must be hidden rather than wrapping over the start, got [{min:?}, {max:?}]"
		);
	}
}
