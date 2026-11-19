//! Effects that resample the content from elsewhere than nearby, like blurs along lines and arcs, and the frame of reference that those relative to the content's extent take.
//! The frame is the content's bounds in its own space, carried through its transform, so a frame-relative effect moves rigidly with the content.

use crate::effects::{EffectOperation, Effects, effect_space, spans_along_axes};
use core_types::graphene_hash::CacheHash;
use dyn_any::DynAny;
use glam::{DAffine2, DMat2, DVec2};

/// How a radial blur smears the content about its center.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash, CacheHash, DynAny, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum RadialBlurMethod {
	/// Smears along circles around the center, as if the content were spinning.
	#[default]
	Spin,
	/// Smears along lines through the center, as if the content were rushing closer.
	Zoom,
}

/// For PSD interop, the motion blur angle and distance matching PSD's whole-degree `angle` and `distance`, whose line PSD rounds to whole pixels.
pub fn motion_blur_from_psd(angle: i32, distance: f64) -> (f64, f64) {
	let angle = (angle as f64).to_radians();
	let end = DVec2::new((distance * angle.cos() + 0.5).trunc(), (distance * angle.sin() + 0.5).trunc());
	(end.y.atan2(end.x).to_degrees(), end.length())
}

/// For PSD interop, the degrees a spin blur's arcs span for each unit of its amount.
pub const SPIN_DEGREES_PER_AMOUNT: f64 = 1.;

/// For PSD interop, the share of each point's distance from the center that a zoom blur's lines span (half toward the center and half away), for each unit of its amount.
pub const ZOOM_FRACTION_PER_AMOUNT: f64 = 0.005;

/// How many samples a radial blur averages for each pixel along its arcs or lines, enough to follow PSD's integral of them to within a level.
pub const RADIAL_SAMPLES_PER_PIXEL: f64 = 2.;

/// The most samples a radial blur averages at one pixel, enough for all but the longest arcs.
pub const MAX_RADIAL_SAMPLES: u32 = 512;

/// The largest amount a radial blur takes: a spin around the whole circle, or a zoom from a tenth of each point's distance from the center to almost twice it.
pub const MAX_RADIAL_BLUR_AMOUNT: f64 = 360.;

/// The placement of the unit square on `bounds` in a space that `transform` maps into another, which is the frame of content with those bounds in its own space.
pub fn frame_of_bounds([min, max]: [DVec2; 2], transform: DAffine2) -> DAffine2 {
	transform * DAffine2::from_translation(min) * DAffine2::from_scale(max - min)
}

/// The frame of reference of an effect relative to the content's extent: a parallelogram placed in the pixels the effect runs on.
/// Positions within it are measured in the effect space's units from its center along its sides, its local space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectFrame {
	/// Places the frame's unit square in the pixels.
	pub to_pixels: DAffine2,
	/// How long the frame's sides are, in the effect space.
	pub size: DVec2,
}

impl EffectFrame {
	/// The frame whose unit square `placement` puts in the effect space, in the pixels that `effect_to_pixels` maps that space into. `None` where it collapses.
	pub fn new(placement: DAffine2, effect_to_pixels: DAffine2) -> Option<Self> {
		let size = DVec2::new(placement.matrix2.x_axis.length(), placement.matrix2.y_axis.length());
		let to_pixels = effect_to_pixels * placement;
		(placement.is_finite() && to_pixels.matrix2.determinant().is_normal()).then_some(Self { to_pixels, size })
	}

	/// The bounding box of the frame in the pixels.
	pub fn pixel_bounds(&self) -> [DVec2; 2] {
		let corners = [DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE].map(|corner| self.to_pixels.transform_point2(corner));
		[corners.into_iter().reduce(DVec2::min).unwrap_or_default(), corners.into_iter().reduce(DVec2::max).unwrap_or_default()]
	}

	/// This frame in other pixels, which `transform` maps these pixels into.
	pub fn placed(&self, transform: DAffine2) -> Self {
		Self {
			to_pixels: transform * self.to_pixels,
			size: self.size,
		}
	}

	/// The map from the frame's local space into the pixels.
	pub fn local_to_pixels(&self) -> DAffine2 {
		self.to_pixels * DAffine2::from_translation(DVec2::splat(0.5)) * DAffine2::from_scale(self.size.recip())
	}

	/// The map from the pixels into the frame's local space.
	pub fn pixels_to_local(&self) -> DAffine2 {
		self.local_to_pixels().inverse()
	}

	/// Whether a point in the local space lies within the frame.
	pub fn contains(&self, local: DVec2) -> bool {
		let half = self.size / 2.;
		local.x.abs() <= half.x && local.y.abs() <= half.y
	}
}

/// A radial blur's arcs or lines, in the local space of its frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialBlur {
	pub method: RadialBlurMethod,
	/// The point the blur spins around or zooms toward.
	pub center: DVec2,
	/// The angle a spin's arcs span, in radians, or the share of each point's distance from the center that a zoom's lines span, each centered on the point.
	pub extent: f64,
}

impl RadialBlur {
	/// The blur of `amount` (up to [`MAX_RADIAL_BLUR_AMOUNT`]) about the `center`, which is a percentage of the size of a frame `size` across away from its middle.
	pub fn new(method: RadialBlurMethod, amount: f64, center: DVec2, size: DVec2) -> Self {
		let amount = amount.clamp(0., MAX_RADIAL_BLUR_AMOUNT);
		let extent = match method {
			RadialBlurMethod::Spin => (amount * SPIN_DEGREES_PER_AMOUNT).to_radians(),
			RadialBlurMethod::Zoom => amount * ZOOM_FRACTION_PER_AMOUNT,
		};

		Self {
			method,
			center: center / 100. * size,
			extent,
		}
	}

	/// How far past the content the blur can paint, given the `farthest` the content reaches from its center: a spin's arcs carry that point along a chord, and a zoom's lines carry it outward.
	pub fn reach(&self, farthest: f64) -> f64 {
		match self.method {
			RadialBlurMethod::Spin => 2. * farthest * (self.extent.min(std::f64::consts::TAU) / 4.).sin(),
			RadialBlurMethod::Zoom => farthest * (self.extent / 2.) / (1. - self.extent / 2.),
		}
	}

	/// How many samples the point at `local` averages, spread along its arc or line by [`RADIAL_SAMPLES_PER_PIXEL`], given how many pixels each unit of the effect space spans.
	pub fn sample_count(&self, local: DVec2, pixels_per_length: f64) -> u32 {
		let length = (local - self.center).length() * self.extent * pixels_per_length * RADIAL_SAMPLES_PER_PIXEL;
		if !length.is_finite() {
			return 1;
		}
		((length.ceil() + 1.) as u32).clamp(1, MAX_RADIAL_SAMPLES)
	}

	/// Sample `index` of the `count` the point at `local` averages, each in the middle of an equal share of its arc or line.
	pub fn sample(&self, local: DVec2, index: u32, count: u32) -> DVec2 {
		self.point_along(local, (index as f64 + 0.5) / count.max(1) as f64 - 0.5)
	}

	/// The point a share `along` the arc or line through the point at `local`, which centers on it, from -0.5 at one end to 0.5 at the other.
	pub fn point_along(&self, local: DVec2, along: f64) -> DVec2 {
		let offset = local - self.center;

		match self.method {
			RadialBlurMethod::Spin => self.center + DVec2::from_angle(self.extent * along).rotate(offset),
			RadialBlurMethod::Zoom => self.center + offset * (1. - self.extent * along),
		}
	}
}

/// How many samples per side of a region its boundary is traced with to find what a frame-relative effect reads.
const TRACED_SAMPLES_PER_SIDE: usize = 64;

/// The bounding box, in pixels, of the points that `sources` gives for each of many points along the boundary of `region` (in pixels), padded to cover what the samples between them miss.
fn traced_bounds(region: [DVec2; 2], sources: impl Fn(DVec2, &mut dyn FnMut(DVec2))) -> [DVec2; 2] {
	let [min, max] = region;
	let corners = [min, DVec2::new(max.x, min.y), max, DVec2::new(min.x, max.y)];

	let mut bounds = [DVec2::INFINITY, DVec2::NEG_INFINITY];
	let mut include = |point: DVec2| {
		if point.is_finite() {
			bounds = [bounds[0].min(point), bounds[1].max(point)];
		}
	};
	for side in 0..4 {
		let (start, end) = (corners[side], corners[(side + 1) % 4]);
		for step in 0..TRACED_SAMPLES_PER_SIDE {
			sources(start.lerp(end, step as f64 / TRACED_SAMPLES_PER_SIDE as f64), &mut include);
		}
	}
	if !(bounds[0].x <= bounds[1].x && bounds[0].y <= bounds[1].y) {
		return region;
	}

	// Between neighboring samples, the traced curve strays little from the straight line through them
	let margin = 2. + 0.02 * (bounds[1] - bounds[0]).max_element();
	[bounds[0] - margin, bounds[1] + margin]
}

fn union([a_min, a_max]: [DVec2; 2], [b_min, b_max]: [DVec2; 2]) -> [DVec2; 2] {
	[a_min.min(b_min), a_max.max(b_max)]
}

impl EffectOperation {
	/// Whether this operation reads from anywhere in its frame, rather than only within its reach around each point.
	pub fn reads_across_frame(&self) -> bool {
		matches!(self, Self::RadialBlur { .. })
	}

	/// The part of its input, in pixels, that this frame-relative operation reads to produce `output` (in pixels), given its `frame`.
	fn source_across_frame(&self, output: [DVec2; 2], frame: &EffectFrame, pixels_per_length: f64) -> [DVec2; 2] {
		let (to_local, to_pixels) = (frame.pixels_to_local(), frame.local_to_pixels());

		let read = match self {
			Self::RadialBlur { method, amount, center, .. } => {
				let blur = RadialBlur::new(*method, *amount, *center, frame.size);

				// A spin turns the region rigidly and a zoom scales it, so its boundary's samples bound all of them
				traced_bounds(output, |point, include| {
					let local = to_local.transform_point2(point);
					let steps = blur.sample_count(local, pixels_per_length).clamp(2, 33) - 1;
					for step in 0..=steps {
						include(to_pixels.transform_point2(blur.point_along(local, step as f64 / steps as f64 - 0.5)));
					}
				})
			}
			_ => output,
		};

		union(output, read)
	}

	/// How far past the content this operation can paint, in its effect space, given the `size` of the content's bounds and how far the effects before it have `painted` past them there.
	/// Most reach a fixed distance, but those relative to the content's extent, like a radial blur, reach farther from larger content.
	fn reach_around(&self, size: DVec2, painted: f64) -> f64 {
		match self {
			// The blur's center lies within its share of the frame's sides from the middle, each side spanning at most the bounds' diagonal
			Self::RadialBlur { method, amount, center, .. } => {
				let diagonal = size.length();
				let farthest = (center.x.abs() + center.y.abs()) / 100. * diagonal + diagonal / 2. + painted;
				RadialBlur::new(*method, *amount, DVec2::ZERO, size).reach(farthest)
			}
			_ => self.painted_reach(),
		}
	}

	/// Whether this operation takes the content's frame, which a radial blur centers in and a motion blur measures its shears from, unless it changes nothing.
	pub fn takes_frame(&self) -> bool {
		match self {
			Self::MotionBlur { .. } | Self::RadialBlur { .. } => !self.changes_nothing(),
			Self::DropShadow { .. }
			| Self::InnerShadow { .. }
			| Self::OuterGlow { .. }
			| Self::InnerGlow { .. }
			| Self::Satin { .. }
			| Self::Outline { .. }
			| Self::BevelAndEmboss { .. }
			| Self::PaintOverlay { .. }
			| Self::Blur { .. }
			| Self::Mask { .. }
			| Self::Feather { .. }
			| Self::ConvolutionKernel { .. }
			| Self::Emboss { .. }
			| Self::FindEdges
			| Self::MinimumMaximum { .. }
			| Self::Sharpen { .. }
			| Self::HighPass { .. }
			| Self::AddNoise { .. }
			| Self::Dissolve { .. }
			| Self::Adjustment(_) => false,
		}
	}
}

/// A bound on how far a unit length in one space stretches in another, given the linear map between them.
fn stretch_bound(linear_map: DMat2) -> f64 {
	let spans = spans_along_axes(linear_map);
	(spans.x * spans.x + spans.y * spans.y).sqrt()
}

/// The content's bounds `size` and `overhang` past its frame, in a space that `space` maps an effect space into, carried into that effect space.
/// The overhang there is as far as it can stretch, so content isn't cut short.
fn measured_in_effect_space(space: DMat2, size: DVec2, overhang: f64) -> (DVec2, f64) {
	let to_effect = if space.determinant().is_normal() { space.inverse() } else { DMat2::IDENTITY };
	(DMat2::from_cols(to_effect.x_axis.abs(), to_effect.y_axis.abs()) * size, overhang * stretch_bound(to_effect))
}

impl Effects {
	/// How far past the content the effects of one run can paint, in their effect space, which `space` maps into the space the content's bounds are measured in,
	/// given the `size` of those bounds and how far the content already paints past its frame there (its `overhang`).
	pub fn run_reach_around(&self, space: DMat2, size: DVec2, overhang: f64) -> f64 {
		let (size, overhang) = measured_in_effect_space(space, size, overhang);
		(self.operations())
			.filter(|operation| !operation.changes_nothing())
			.fold(0., |painted, operation| painted + operation.reach_around(size, overhang + painted))
	}

	/// How far past the content the effects together can paint along each axis of the space `content_transform` places the content in, given the `size` of the content's bounds there,
	/// past which effects relative to the content's extent reach farther from larger content. The content's `overhang` is how far it already paints past its frame there, as nested effects do.
	pub fn reach_around(&self, content_transform: DAffine2, size: DVec2, overhang: f64) -> DVec2 {
		self.reach_around_by(content_transform, size, overhang, |_| true)
	}

	/// Like [`Effects::reach_around`], counting only how far those reading the pixels around each one of the image they work on read it (see [`EffectOperation::reads_pixels_around`]),
	/// along the axis they read furthest along.
	pub fn read_reach_around(&self, content_transform: DAffine2, size: DVec2, overhang: f64) -> f64 {
		let painted = self.reach_around_by(content_transform, size, overhang, EffectOperation::reads_pixels_around);
		(painted + self.read_beyond_painted(content_transform)).max_element()
	}

	/// How much farther than they paint the effects reading around each pixel read, like a kernel, along each axis of the space `content_transform` places the content in.
	/// A render holds this much past the pixels it needs right, even those in view.
	pub fn read_beyond_painted(&self, content_transform: DAffine2) -> DVec2 {
		self.reach_along_axes_by(content_transform, |operation| operation.required_reach() - operation.painted_reach())
	}

	/// Like [`Effects::reach_around`], counting only the effects that `counts` accepts, though every effect paints what those after it measure from.
	/// Each run measures the content's size and what's painted before it in its effect space.
	pub fn reach_around_by(&self, content_transform: DAffine2, size: DVec2, overhang: f64, counts: impl Fn(&EffectOperation) -> bool) -> DVec2 {
		let (mut painted, mut counted) = (DVec2::ZERO, DVec2::ZERO);

		for (stamped_transform, rows) in self.runs() {
			let space = effect_space(content_transform, stamped_transform).matrix2;
			let (size, before) = measured_in_effect_space(space, size, overhang + painted.max_element());
			let (mut painted_in_run, mut counted_in_run) = (before, 0.);
			for operation in self.operations_in(rows).filter(|operation| !operation.changes_nothing()) {
				let reach = operation.reach_around(size, painted_in_run);
				painted_in_run += reach;
				if counts(&operation) {
					counted_in_run += reach;
				}
			}

			let spans = spans_along_axes(space);
			painted += (painted_in_run - before) * spans;
			counted += counted_in_run * spans;
		}

		counted
	}

	/// Whether any of the effects takes the content's frame, which is costly to find for a stroked shape or a group.
	pub fn take_frame(&self) -> bool {
		self.operations().any(|operation| operation.takes_frame())
	}

	/// Whether how far the effects reach depends on the size of the content's bounds, as a radial blur's does.
	pub fn reach_depends_on_size(&self) -> bool {
		self.operations()
			.any(|operation| matches!(operation, EffectOperation::RadialBlur { .. }) && !operation.changes_nothing())
	}

	/// The region, in pixels, that the content has to be rendered over so the effects can produce the `visible` part of their result (in pixels),
	/// if any of them reads from across its `frame` rather than only within its reach. `lengths_to_pixels` maps the effect space's lengths into the pixels.
	/// The stages before each effect may need different regions, all of which the result covers, since the effects' passes share one image.
	pub fn source_region(&self, visible: [DVec2; 2], frame: Option<&EffectFrame>, lengths_to_pixels: DMat2) -> Option<[DVec2; 2]> {
		let frame = frame?;
		let operations: Vec<EffectOperation> = self.operations().filter(|operation| !operation.changes_nothing()).collect();
		if !operations.iter().any(EffectOperation::reads_across_frame) {
			return None;
		}

		let pixels_per_length = lengths_to_pixels.determinant().abs().sqrt();
		let spans = spans_along_axes(lengths_to_pixels);

		// Folded backward from the visible result, each effect needing what it reads to produce the region the next one needs
		let mut region = visible;
		let mut covered = visible;
		for operation in operations.iter().rev() {
			region = match operation.reads_across_frame() {
				true => operation.source_across_frame(region, frame, pixels_per_length),
				false => {
					let reach = operation.reach() * spans;
					[region[0] - reach, region[1] + reach]
				}
			};
			covered = union(covered, region);
		}

		Some(covered)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::effects::EffectOperation;

	fn frame() -> EffectFrame {
		EffectFrame::new(frame_of_bounds([DVec2::new(100., 100.), DVec2::new(300., 200.)], DAffine2::IDENTITY), DAffine2::IDENTITY).unwrap()
	}

	#[test]
	fn frames_map_their_local_space_from_the_middle() {
		let frame = frame();
		assert!((frame.local_to_pixels().transform_point2(DVec2::ZERO) - DVec2::new(200., 150.)).length() < 1e-9);
		assert!((frame.local_to_pixels().transform_point2(DVec2::new(100., -50.)) - DVec2::new(300., 100.)).length() < 1e-9);
		assert!(frame.contains(DVec2::new(99., 49.)));
		assert!(!frame.contains(DVec2::new(101., 0.)));
		assert!(EffectFrame::new(frame_of_bounds([DVec2::ZERO, DVec2::new(0., 5.)], DAffine2::IDENTITY), DAffine2::IDENTITY).is_none());

		// Turned and scaled with its content, the frame keeps its sides' lengths in the effect space
		let turned = EffectFrame::new(frame_of_bounds([DVec2::ZERO, DVec2::new(40., 10.)], DAffine2::from_angle(0.5)), DAffine2::from_scale(DVec2::splat(2.))).unwrap();
		assert!((turned.size - DVec2::new(40., 10.)).length() < 1e-9);
		let right_middle = turned.local_to_pixels().transform_point2(DVec2::new(20., 0.));
		assert!((right_middle - DVec2::from_angle(0.5).rotate(DVec2::new(80., 10.))).length() < 1e-9, "{right_middle}");
	}

	#[test]
	fn spins_sample_along_arcs_centered_on_each_point() {
		let blur = RadialBlur::new(RadialBlurMethod::Spin, 90. / SPIN_DEGREES_PER_AMOUNT, DVec2::ZERO, frame().size);
		let point = DVec2::new(10., 0.);
		let count = blur.sample_count(point, 1.);

		// An arc of a quarter turn at radius 10 is about 16 pixels long, sampled twice per pixel
		assert_eq!(count, 33);
		assert!((blur.sample(point, count / 2, count) - point).length() < 1e-9, "the middle sample is the point itself");
		assert!((blur.point_along(point, -0.5) - DVec2::from_angle(-std::f64::consts::FRAC_PI_4) * 10.).length() < 1e-9);

		// Each sample lies in the middle of its share of the arc, so the first lies half a share within the arc's end
		let first = DVec2::from_angle(-std::f64::consts::FRAC_PI_4 * (1. - 1. / count as f64)) * 10.;
		assert!((blur.sample(point, 0, count) - first).length() < 1e-9);

		// The samples of a whole turn spread evenly around the circle, so no two land together and they balance about the center
		let turn = RadialBlur::new(RadialBlurMethod::Spin, 360. / SPIN_DEGREES_PER_AMOUNT, DVec2::ZERO, frame().size);
		let count = turn.sample_count(point, 1.);
		let mean = (0..count).map(|index| turn.sample(point, index, count)).sum::<DVec2>() / count as f64;
		assert!(mean.length() < 1e-9, "the samples of a whole turn average to {mean}");
	}

	#[test]
	fn zooms_sample_along_lines_through_each_point_toward_the_center() {
		let blur = RadialBlur::new(RadialBlurMethod::Zoom, 0.5 / ZOOM_FRACTION_PER_AMOUNT, DVec2::new(50., 0.), frame().size);
		assert_eq!(blur.center, DVec2::new(100., 0.), "the center is a percentage of the frame's size from its middle");

		// For PSD interop, the line spans from three quarters to five quarters of the point's distance from the center
		let point = DVec2::new(100., 40.);
		assert!((blur.point_along(point, -0.5) - DVec2::new(100., 50.)).length() < 1e-9);
		assert!((blur.point_along(point, 0.5) - DVec2::new(100., 30.)).length() < 1e-9);

		// Its samples lie in the middles of equal shares of the line, half a share within each end
		let count = blur.sample_count(point, 1.);
		let half_share = 10. / count as f64;
		assert!((blur.sample(point, 0, count) - DVec2::new(100., 50. - half_share)).length() < 1e-9);
		assert!((blur.sample(point, count / 2, count) - point).length() < 1e-9, "the middle sample is the point itself");
		assert!((blur.sample(point, count - 1, count) - DVec2::new(100., 30. + half_share)).length() < 1e-9);
	}

	#[test]
	fn a_spin_reads_beyond_the_visible_part_of_its_result() {
		let effects = Effects::from_operations([EffectOperation::RadialBlur {
			method: RadialBlurMethod::Spin,
			amount: 60.,
			center: DVec2::ZERO,
			gamma: false,
		}]);

		// Only the frame's top right corner is visible, whose arcs sweep around past its left and bottom
		let visible = [DVec2::new(250., 100.), DVec2::new(300., 130.)];
		let [min, max] = effects.source_region(visible, Some(&frame()), DMat2::IDENTITY).unwrap();
		assert!(min.x < 230. && max.y > 150., "the region reaches from {min} to {max}");
		assert!(min.x <= 250. && min.y <= 100. && max.x >= 300. && max.y >= 130., "the region covers the visible part");

		// Effects that read only within their reach need nothing beyond it
		let blur = Effects::from_operations([EffectOperation::default_for(crate::EffectKind::Blur)]);
		assert_eq!(blur.source_region(visible, Some(&frame()), DMat2::IDENTITY), None);
	}

	#[test]
	fn radial_blurs_reach_farther_from_larger_content() {
		let radial = |method| {
			Effects::from_operations([EffectOperation::RadialBlur {
				method,
				amount: 360.,
				center: DVec2::ZERO,
				gamma: false,
			}])
		};
		let half_diagonal = 50. * std::f64::consts::SQRT_2;

		// A full turn sweeps the corners of a square 100 across around its whole circle, as far as the diagonal
		let spin = radial(RadialBlurMethod::Spin);
		let reach = |effects: &Effects, content_transform, size| effects.reach_around(content_transform, DVec2::splat(size), 0.);
		assert!((reach(&spin, DAffine2::IDENTITY, 100.).x - 2. * half_diagonal).abs() < 1e-9);
		assert!((reach(&spin, DAffine2::IDENTITY, 200.).x - 4. * half_diagonal).abs() < 1e-9);

		// Scaled after it's applied, the spin measures the content in its effect space and reaches as far scaled along with it
		assert!((reach(&spin, DAffine2::from_scale(DVec2::splat(3.)), 300.) - 3. * reach(&spin, DAffine2::IDENTITY, 100.)).length() < 1e-9);

		// A zoom of 360 spans from a tenth to nineteen tenths of each point's distance, so it carries the farthest corner out ninefold
		let zoom = radial(RadialBlurMethod::Zoom);
		assert!((reach(&zoom, DAffine2::IDENTITY, 100.).x - 9. * half_diagonal).abs() < 1e-6);
	}
}
