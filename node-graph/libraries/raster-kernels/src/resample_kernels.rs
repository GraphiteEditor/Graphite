//! The effects that resample the content from elsewhere than nearby, on the CPU: blurs along lines and arcs.
//! They mirror the renderer's GPU passes, sampling between pixels the same way.

use crate::effect_kernels::Plane;
use glam::{DMat2, DVec2};
use graphic_types::resampling::{EffectFrame, RadialBlur};
use raster_types::parallel_rows::fill_rows;

/// The most samples a motion blur's box averages along its line at each pixel, past which its samples spread out evenly to cover it.
pub const MAX_MOTION_BLUR_SAMPLES: f64 = 1024.;

/// For PSD interop, a motion blur shears the image so its line runs along whichever axis it's closer to (the major axis), averages a box along that axis, and shears back.
/// Each shear interpolates across the minor axis with [`keys_cubic`], measured from the frame's edge as PSD measures from the image's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionBlurShear {
	/// Whether the line is closer to vertical, so the shears move columns of pixels across rows rather than rows across columns.
	pub vertical: bool,
	/// How far the line moves across the minor axis for each pixel along the major axis.
	pub slope: f64,
	/// How many pixels the line spans along the major axis.
	pub length: f64,
	/// The fraction of a pixel the shears move the column (or row) at the start of the major axis back by, so they're measured from the anchor.
	pub phase: f64,
	/// Which way along the major axis the box leans at odd lengths: 1 toward its far end, or -1 toward its near end.
	pub lean: f64,
}

impl MotionBlurShear {
	/// The shears of a line `distance` long at `angle` (in degrees counterclockwise from the right), both in the effect space, given the map from its lengths into pixels, measured from `anchor` (in pixels).
	pub fn new(angle: f64, distance: f64, lengths_to_pixels: DMat2, anchor: DVec2) -> Option<Self> {
		let angle = angle.to_radians();
		let line = DVec2::new(angle.cos(), -angle.sin()) * distance.max(0.);
		let end = lengths_to_pixels * line;
		let vertical = end.y.abs() > end.x.abs();
		let (major, minor, anchor) = if vertical { (end.y, end.x, anchor.y) } else { (end.x, end.y, anchor.x) };
		if !(major.abs() >= 0.01 && major.is_finite() && minor.is_finite() && anchor.is_finite()) {
			return None;
		}

		// A line shorter than a pixel slants in only as it lengthens, as PSD's whole-pixel line stays straight along an axis until it reaches across
		let slope = minor / major * major.abs().min(1.);

		// The box leans toward the end of the line farther along the effect space's axis nearest it, so it leans alike however the pixels lie in that space
		let along_own_axis = if line.y.abs() > line.x.abs() { line.y } else { line.x };
		let lean = if (major > 0.) == (along_own_axis > 0.) { 1. } else { -1. };

		Some(Self {
			vertical,
			slope,
			length: major.abs(),
			phase: (slope * anchor).rem_euclid(1.),
			lean,
		})
	}

	/// How far the shear moves the column (or row) whose center lies at `center` along the major axis, as whole pixels and the fraction left over.
	pub fn shift(&self, center: f64) -> (i64, f64) {
		let shift = self.slope * center - self.phase;
		let whole = shift.floor();
		(whole as i64, shift - whole)
	}

	/// The span of the box along the major axis around each pixel: one pixel longer than the line, and for PSD interop, half a pixel toward the end of the axis it leans to at odd lengths.
	pub fn box_span(&self) -> [f64; 2] {
		let center = self.lean * (0.5 - (0.5 - (self.length / 2.).fract()).abs());
		let half = (self.length + 1.) / 2.;
		[center - half, center + half]
	}

	/// How many pixels apart the box's samples are, so it takes no more than [`MAX_MOTION_BLUR_SAMPLES`].
	pub fn stride(&self) -> f64 {
		((self.length + 2.) / MAX_MOTION_BLUR_SAMPLES).ceil().max(1.)
	}

	/// The box's samples, as their offsets in pixels along the major axis and their weights: the share of the box each one's stretch of the line covers.
	pub fn box_samples(&self) -> Vec<(i64, f32)> {
		let [start, end] = self.box_span();
		let stride = self.stride();

		let first = ((start + stride / 2.) / stride).floor() as i64;
		let last = ((end - stride / 2.) / stride).ceil() as i64;
		(first..=last)
			.filter_map(|index| {
				let offset = index as f64 * stride;
				let weight = end.min(offset + stride / 2.) - start.max(offset - stride / 2.);
				(weight > 0.).then_some((offset as i64, weight as f32))
			})
			.collect()
	}
}

/// The Keys cubic convolution kernel with a = -0.75, which PSD's shears interpolate with.
pub fn keys_cubic(x: f64) -> f64 {
	const A: f64 = -0.75;
	let x = x.abs();
	match x {
		_ if x < 1. => (A + 2.) * x * x * x - (A + 3.) * x * x + 1.,
		_ if x < 2. => A * x * x * x - 5. * A * x * x + 8. * A * x - 4. * A,
		_ => 0.,
	}
}

/// Premultiplied channels clipped to what they can hold: alpha from 0 to 1, and color from 0 to the alpha.
fn clipped([red, green, blue, alpha]: [f32; 4]) -> [f32; 4] {
	let alpha = alpha.clamp(0., 1.);
	[red.clamp(0., alpha), green.clamp(0., alpha), blue.clamp(0., alpha), alpha]
}

/// The plane smeared along the motion blur's line by its two shears around a box. O(width × height × samples).
pub fn motion_blurred(plane: &Plane<[f32; 4]>, shear: &MotionBlurShear) -> Plane<[f32; 4]> {
	// Pixels by their position along the major axis and across it, with transparency past the edges
	let (majors, minors) = if shear.vertical { (plane.height, plane.width) } else { (plane.width, plane.height) };
	let index = |major: i64, minor: i64| match shear.vertical {
		true => minor as usize + major as usize * plane.width,
		false => major as usize + minor as usize * plane.width,
	};
	let at = |plane: &Plane<[f32; 4]>, major: i64, minor: i64| match major >= 0 && minor >= 0 && (major as usize) < majors && (minor as usize) < minors {
		true => plane.data[index(major, minor)],
		false => [0.; 4],
	};
	let weighted_sum = |samples: &mut dyn Iterator<Item = ([f32; 4], f32)>| samples.fold([0.; 4], |sum, (sample, weight)| std::array::from_fn(|channel| sum[channel] + sample[channel] * weight));
	let each_pixel = |value: &(dyn Fn(i64, i64) -> [f32; 4] + Sync)| along_axes(plane.width, plane.height, shear.vertical, value);
	let shift = |major: i64| shear.shift(major as f64 + 0.5);

	// Each column (or row) moves across the minor axis by the fraction of its shift, leaving the whole pixels for the box to step over
	let sheared = each_pixel(&|major, minor| {
		let (_, fraction) = shift(major);
		clipped(weighted_sum(
			&mut (-1..=2).map(|offset| (at(plane, major, minor + offset), keys_cubic(offset as f64 - fraction) as f32)),
		))
	});

	// Averaged along the line, which steps across the minor axis by the whole pixels of each column's shift
	let samples = shear.box_samples();
	let total: f32 = samples.iter().map(|(_, weight)| weight).sum();
	// Samples farther along than the plane is long read only the transparency past its edges
	let samples: Vec<(i64, f32)> = samples.into_iter().filter(|(offset, _)| offset.unsigned_abs() < majors as u64).collect();
	let boxed = each_pixel(&|major, minor| {
		let (whole, _) = shift(major);
		let sum = weighted_sum(&mut samples.iter().map(|&(offset, weight)| (at(&sheared, major + offset, minor + shift(major + offset).0 - whole), weight)));
		sum.map(|channel| channel / total)
	});

	// Moved back by the fraction of the shift
	each_pixel(&|major, minor| {
		let (_, fraction) = shift(major);
		clipped(weighted_sum(
			&mut (-2..=1).map(|offset| (at(&boxed, major, minor + offset), keys_cubic(offset as f64 + fraction) as f32)),
		))
	})
}

/// A plane of `width` and `height` whose pixels are `value` at their positions along the major axis and across it, which are y and x if `vertical`.
fn along_axes(width: usize, height: usize, vertical: bool, value: &(dyn Fn(i64, i64) -> [f32; 4] + Sync)) -> Plane<[f32; 4]> {
	let mut result = Plane::new(width, height);
	fill_rows(&mut result.data, width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			*pixel = if vertical { value(y as i64, x as i64) } else { value(x as i64, y as i64) };
		}
	});
	result
}

/// The premultiplied pixel at a `point` in pixels, interpolated between the four pixel centers around it, with transparency beyond the edges.
pub fn sample_bilinear(plane: &Plane<[f32; 4]>, point: DVec2) -> [f32; 4] {
	let position = point - 0.5;
	let base = position.floor();
	let fraction = (position - base).as_vec2();
	let (x, y) = (base.x as i64, base.y as i64);

	let at = |x: i64, y: i64| match x >= 0 && y >= 0 && (x as usize) < plane.width && (y as usize) < plane.height {
		true => plane.data[y as usize * plane.width + x as usize],
		false => [0.; 4],
	};
	let mix = |a: [f32; 4], b: [f32; 4], t: f32| std::array::from_fn(|channel| a[channel] + (b[channel] - a[channel]) * t);

	let top = mix(at(x, y), at(x + 1, y), fraction.x);
	let bottom = mix(at(x, y + 1), at(x + 1, y + 1), fraction.x);
	mix(top, bottom, fraction.y)
}

/// The plane blurred along arcs around the radial blur's center or lines through it, placed in the local space of its `frame`.
/// `pixels_per_length` is how many pixels each unit of the effect space spans. O(width × height × samples).
pub fn radial_blurred(plane: &Plane<[f32; 4]>, blur: &RadialBlur, frame: &EffectFrame, pixels_per_length: f64) -> Plane<[f32; 4]> {
	let (to_local, to_pixels) = (frame.pixels_to_local(), frame.local_to_pixels());

	let mut blurred = Plane::new(plane.width, plane.height);
	fill_rows(&mut blurred.data, plane.width, |y, row| {
		for (x, pixel) in row.iter_mut().enumerate() {
			let local = to_local.transform_point2(DVec2::new(x as f64 + 0.5, y as f64 + 0.5));
			let count = blur.sample_count(local, pixels_per_length);
			let mut sum = [0.; 4];
			for index in 0..count {
				let sample = sample_bilinear(plane, to_pixels.transform_point2(blur.sample(local, index, count)));
				sum = std::array::from_fn(|channel| sum[channel] + sample[channel]);
			}
			*pixel = sum.map(|channel| channel / count as f32);
		}
	});
	blurred
}

#[cfg(test)]
mod tests {
	use super::*;

	fn shear(angle: f64, distance: f64) -> MotionBlurShear {
		MotionBlurShear::new(angle, distance, DMat2::IDENTITY, DVec2::ZERO).unwrap()
	}

	/// A plane 41 pixels square, transparent but for an opaque white point in the middle, smeared by the motion blur.
	fn smeared_point(shear: MotionBlurShear) -> impl Fn(usize, usize) -> f32 {
		let mut plane = Plane::<[f32; 4]>::new(41, 41);
		plane.data[20 * 41 + 20] = [1.; 4];
		let blurred = motion_blurred(&plane, &shear);
		move |x, y| blurred.data[y * 41 + x][3]
	}

	#[test]
	fn boxes_span_one_pixel_more_than_the_line_leaning_forward_at_odd_lengths() {
		assert_eq!(shear(0., 10.).box_span(), [-5.5, 5.5]);
		assert_eq!(shear(0., 11.).box_span(), [-5.5, 6.5]);
		assert_eq!(shear(0., 11.).box_samples().len(), 12);

		// Between whole lengths, the end samples weigh the share of them the box covers
		let samples = shear(0., 10.5).box_samples();
		let total: f32 = samples.iter().map(|(_, weight)| weight).sum();
		assert!((total - 11.5).abs() < 1e-5);
	}

	#[test]
	fn a_motion_blur_smears_a_point_evenly_along_its_line() {
		// A vertical line ten pixels long spreads the point over eleven pixels
		let at = smeared_point(shear(90., 10.));
		for y in 15..=25 {
			assert!((at(20, y) - 1. / 11.).abs() < 1e-6, "row {y} holds {}", at(20, y));
		}
		assert_eq!(at(20, 14), 0.);
		assert_eq!(at(21, 20), 0.);

		// For PSD interop, a diagonal line seven pixels along leans forward, and shears measured from a pixel's center move it by whole pixels
		let (angle, distance) = graphic_types::resampling::motion_blur_from_psd(45, 10.);
		let at = smeared_point(MotionBlurShear::new(angle, distance, DMat2::IDENTITY, DVec2::splat(0.5)).unwrap());
		for step in -3..=4_i32 {
			let (x, y) = ((20 - step) as usize, (20 + step) as usize);
			assert!((at(x, y) - 1. / 8.).abs() < 1e-5, "({x}, {y}) holds {}", at(x, y));
		}
		assert!(at(24, 16) < 1e-6 && at(15, 25) < 1e-6);
	}

	#[test]
	fn motion_blurs_flip_and_turn_with_the_pixels_their_effect_space_lies_in() {
		// A plane 31 pixels square holding a few points off its middle
		const SIZE: usize = 31;
		let mut plane = Plane::<[f32; 4]>::new(SIZE, SIZE);
		for (x, y, value) in [(9, 12, 1.), (17, 20, 0.6), (22, 8, 1.), (14, 14, 0.3)] {
			plane.data[y * SIZE + x] = [value; 4];
		}
		let centers = || (0..SIZE * SIZE).map(|index| DVec2::new((index % SIZE) as f64 + 0.5, (index / SIZE) as f64 + 0.5));
		let index_at = |center: DVec2| center.y as usize * SIZE + center.x as usize;

		// Pixels the effect space lies flipped across, flipped down, turned a half, and turned a quarter each way in, about the plane's middle
		let middle = DVec2::splat(SIZE as f64 / 2.);
		let placements = [
			DMat2::from_diagonal(DVec2::new(-1., 1.)),
			DMat2::from_diagonal(DVec2::new(1., -1.)),
			DMat2::from_diagonal(DVec2::splat(-1.)),
			DMat2::from_cols(DVec2::Y, -DVec2::X),
			DMat2::from_cols(-DVec2::Y, DVec2::X),
		];

		// Lines an odd number of pixels long along their major axis, whose boxes lean a whole half pixel, with shears measured from the plane's corner
		let slanted_distance = 9. / 30_f64.to_radians().cos();
		for (angle, distance) in [(0., 5.), (180., 5.), (90., 7.), (30., slanted_distance), (-60., slanted_distance)] {
			let upright = motion_blurred(&plane, &shear(angle, distance));

			for effect_to_pixels in placements {
				let placed = |point: DVec2| effect_to_pixels * (point - middle) + middle;
				let mut placed_plane = Plane::<[f32; 4]>::new(SIZE, SIZE);
				centers().for_each(|center| placed_plane.data[index_at(placed(center))] = plane.data[index_at(center)]);

				// The blur of the placed pixels is the upright blur, placed
				let placed_shear = MotionBlurShear::new(angle, distance, effect_to_pixels, placed(DVec2::ZERO)).unwrap();
				let blurred = motion_blurred(&placed_plane, &placed_shear);
				for center in centers() {
					let (expected, actual) = (upright.data[index_at(center)][3], blurred.data[index_at(placed(center))][3]);
					assert!(
						(expected - actual).abs() < 1e-4,
						"a line at {angle} degrees through pixels placed by {effect_to_pixels} holds {actual} rather than {expected} at {center}"
					);
				}
			}
		}

		// The box leans toward the same end of the line in the effect space, which is the major axis's near end in flipped pixels
		let flipped = MotionBlurShear::new(0., 11., DMat2::from_diagonal(DVec2::new(-1., 1.)), DVec2::ZERO).unwrap();
		assert_eq!(flipped.box_span(), [-6.5, 5.5]);
		assert_eq!(shear(180., 11.).box_span(), shear(0., 11.).box_span(), "opposite directions smear alike");
	}

	#[test]
	fn huge_motion_blurs_spread_their_samples_out() {
		let shear = shear(0., 5000.);
		assert!(shear.box_samples().len() as f64 <= MAX_MOTION_BLUR_SAMPLES + 1.);
		let total: f32 = shear.box_samples().iter().map(|(_, weight)| weight).sum();
		assert!((total - 5001.).abs() < 0.01);
	}

	#[test]
	fn bilinear_samples_fade_to_transparency_past_the_edges() {
		let mut plane = Plane::<[f32; 4]>::new(2, 1);
		plane.data = vec![[1.; 4], [0.5; 4]];

		assert_eq!(sample_bilinear(&plane, DVec2::new(1., 0.5)), [0.75; 4]);
		assert_eq!(sample_bilinear(&plane, DVec2::new(0., 0.5)), [0.5; 4]);
	}
}
