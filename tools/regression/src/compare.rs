//! Compares a render to its reference pixel by pixel.

use crate::corpus::Tolerance;
use image::RgbaImage;

pub enum Comparison {
	Matches,
	SizeChanged {
		reference: (u32, u32),
		actual: (u32, u32),
	},
	/// More pixels than the tolerance allows differ, highlighted in `diff`.
	Differs {
		differing: usize,
		total: usize,
		diff: RgbaImage,
	},
}

/// O(pixels).
pub fn compare(reference: &RgbaImage, actual: &RgbaImage, tolerance: Tolerance) -> Comparison {
	if reference.dimensions() != actual.dimensions() {
		return Comparison::SizeChanged {
			reference: reference.dimensions(),
			actual: actual.dimensions(),
		};
	}

	// Differing pixels show red over a faded copy of the reference, so they stand out in context
	let mut diff = RgbaImage::new(actual.width(), actual.height());
	let mut differing = 0;
	for ((expected, rendered), marked) in reference.pixels().zip(actual.pixels()).zip(diff.pixels_mut()) {
		let largest_difference = expected.0.iter().zip(rendered.0).map(|(a, b)| a.abs_diff(b)).max().unwrap_or_default();
		if largest_difference > tolerance.channel {
			differing += 1;
			*marked = image::Rgba([255, 0, 0, 255]);
		} else {
			let [red, green, blue, alpha] = expected.0;
			let luminance = ((red as u32 * 3 + green as u32 * 6 + blue as u32) / 10) as u8;
			*marked = image::Rgba([luminance, luminance, luminance, alpha / 4]);
		}
	}

	let total = (actual.width() * actual.height()) as usize;
	if differing as f64 > tolerance.pixels * total as f64 {
		Comparison::Differs { differing, total, diff }
	} else {
		Comparison::Matches
	}
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

		let Comparison::Differs { differing, total, diff } = compare(&reference, &one_off, Tolerance::default()) else {
			panic!("one pixel in four exceeds the default tolerance");
		};
		assert_eq!((differing, total), (1, 4));
		assert_eq!(diff.get_pixel(3, 0).0, [255, 0, 0, 255]);
	}

	#[test]
	fn a_size_change_never_matches() {
		let comparison = compare(&image_of(&[[0; 4]; 2]), &image_of(&[[0; 4]; 3]), Tolerance::default());
		assert!(matches!(comparison, Comparison::SizeChanged { reference: (2, 1), actual: (3, 1) }));
	}
}
