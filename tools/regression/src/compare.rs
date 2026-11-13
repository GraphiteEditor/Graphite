//! Compares a render to its reference pixel by pixel.

use crate::corpus::Tolerance;
use image::RgbaImage;
use image::imageops::{self, FilterType};
use image::math::Rect;

/// Pixels of unchanged surroundings kept around the differing region in a side-by-side image.
const CONTEXT_MARGIN: u32 = 32;
/// The widest a side-by-side image gets before it is scaled down.
const MAX_SIDE_BY_SIDE_WIDTH: u32 = 1800;
const PANEL_GAP: u32 = 8;

pub(crate) enum Comparison {
	Matches,
	SizeChanged {
		reference: (u32, u32),
		actual: (u32, u32),
	},
	/// More pixels than the tolerance allows differ, highlighted in `diff` and all within `region`.
	Differs {
		differing: usize,
		total: usize,
		diff: RgbaImage,
		region: Rect,
	},
}

/// O(pixels).
pub(crate) fn compare(reference: &RgbaImage, actual: &RgbaImage, tolerance: Tolerance) -> Comparison {
	if reference.dimensions() != actual.dimensions() {
		return Comparison::SizeChanged {
			reference: reference.dimensions(),
			actual: actual.dimensions(),
		};
	}

	// Differing pixels show red over a faded copy of the reference, so they stand out in context
	let mut diff = RgbaImage::new(actual.width(), actual.height());
	let mut differing = 0;
	let (mut min, mut max) = ((u32::MAX, u32::MAX), (0, 0));
	for ((expected, rendered), (x, y, marked)) in reference.pixels().zip(actual.pixels()).zip(diff.enumerate_pixels_mut()) {
		let largest_difference = expected.0.iter().zip(rendered.0).map(|(a, b)| a.abs_diff(b)).max().unwrap_or_default();
		if largest_difference > tolerance.channel {
			differing += 1;
			min = (min.0.min(x), min.1.min(y));
			max = (max.0.max(x), max.1.max(y));
			*marked = image::Rgba([255, 0, 0, 255]);
		} else {
			let [red, green, blue, alpha] = expected.0;
			let luminance = ((red as u32 * 3 + green as u32 * 6 + blue as u32) / 10) as u8;
			*marked = image::Rgba([luminance, luminance, luminance, alpha / 4]);
		}
	}

	let total = (actual.width() * actual.height()) as usize;
	if differing as f64 > tolerance.pixels * total as f64 {
		let region = Rect {
			x: min.0,
			y: min.1,
			width: max.0 - min.0 + 1,
			height: max.1 - min.1 + 1,
		};
		Comparison::Differs { differing, total, diff, region }
	} else {
		Comparison::Matches
	}
}

/// The reference, render, and diff next to each other, cropped to `region` with some context and scaled down to fit a review comment.
pub(crate) fn side_by_side(reference: &RgbaImage, actual: &RgbaImage, diff: &RgbaImage, region: Rect) -> RgbaImage {
	let x = region.x.saturating_sub(CONTEXT_MARGIN);
	let y = region.y.saturating_sub(CONTEXT_MARGIN);
	let width = (region.x + region.width + CONTEXT_MARGIN).min(reference.width()) - x;
	let height = (region.y + region.height + CONTEXT_MARGIN).min(reference.height()) - y;

	let panels = [reference, actual, diff];
	let mut combined = RgbaImage::new(width * 3 + PANEL_GAP * 2, height);
	for (index, panel) in panels.into_iter().enumerate() {
		let cropped = imageops::crop_imm(panel, x, y, width, height).to_image();
		imageops::overlay(&mut combined, &cropped, (index as u32 * (width + PANEL_GAP)) as i64, 0);
	}

	if combined.width() <= MAX_SIDE_BY_SIDE_WIDTH {
		return combined;
	}
	let scaled_height = (combined.height() as u64 * MAX_SIDE_BY_SIDE_WIDTH as u64 / combined.width() as u64).max(1) as u32;
	imageops::resize(&combined, MAX_SIDE_BY_SIDE_WIDTH, scaled_height, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn image_of(pixels: &[[u8; 4]]) -> RgbaImage {
		RgbaImage::from_raw(pixels.len() as u32, 1, pixels.concat()).unwrap()
	}

	#[test]
	fn small_differences_and_few_pixels_are_tolerated() {
		let reference = image_of(&[[10, 10, 10, 255]; 4]);
		let close = image_of(&[[12, 9, 10, 255]; 4]);
		assert!(matches!(compare(&reference, &close, Tolerance::default()), Comparison::Matches));

		let one_off = image_of(&[[10, 10, 10, 255], [10, 10, 10, 255], [10, 10, 10, 255], [200, 10, 10, 255]]);
		let lenient = Tolerance { channel: 2, pixels: 0.25 };
		assert!(matches!(compare(&reference, &one_off, lenient), Comparison::Matches));

		let Comparison::Differs { differing, total, diff, region } = compare(&reference, &one_off, Tolerance::default()) else {
			panic!("one pixel in four exceeds the default tolerance");
		};
		assert_eq!((differing, total), (1, 4));
		assert_eq!(diff.get_pixel(3, 0).0, [255, 0, 0, 255]);
		assert_eq!((region.x, region.y, region.width, region.height), (3, 0, 1, 1));
	}

	#[test]
	fn side_by_side_crops_each_panel_to_the_differing_region() {
		let reference = RgbaImage::from_pixel(200, 100, image::Rgba([0, 0, 0, 255]));
		let mut actual = reference.clone();
		actual.put_pixel(100, 50, image::Rgba([255, 255, 255, 255]));

		let exact = Tolerance { channel: 0, pixels: 0. };
		let Comparison::Differs { diff, region, .. } = compare(&reference, &actual, exact) else {
			panic!("any changed pixel exceeds an exact tolerance");
		};
		let combined = side_by_side(&reference, &actual, &diff, region);

		let panel_width = CONTEXT_MARGIN * 2 + 1;
		assert_eq!(combined.dimensions(), (panel_width * 3 + PANEL_GAP * 2, CONTEXT_MARGIN * 2 + 1));
		assert_eq!(combined.get_pixel(CONTEXT_MARGIN + panel_width + PANEL_GAP, CONTEXT_MARGIN).0, [255, 255, 255, 255]);
	}

	#[test]
	fn a_size_change_never_matches() {
		let comparison = compare(&image_of(&[[0; 4]; 2]), &image_of(&[[0; 4]; 3]), Tolerance::default());
		assert!(matches!(comparison, Comparison::SizeChanged { reference: (2, 1), actual: (3, 1) }));
	}
}
