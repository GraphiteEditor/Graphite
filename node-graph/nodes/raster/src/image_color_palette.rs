use core_types::color::Color;
use core_types::context::Ctx;
use core_types::list::{Item, List};
use raster_types::{CPU, Raster};

#[node_macro::node(category("Color"))]
async fn image_color_palette(
	_: impl Ctx,
	image: List<Raster<CPU>>,
	#[default(4)]
	#[hard_min(1)]
	count: u32,
) -> List<Color> {
	const GRID: f32 = 3.;

	let bins = GRID * GRID * GRID;

	let mut histogram = vec![0; (bins + 1.) as usize];
	// Each bin stores `(red, green, blue, alpha)` tuples in sRGB gamma space; averaging in gamma space gives perceptually-uniform binning.
	let mut color_bins: Vec<Vec<[f32; 4]>> = vec![Vec::new(); (bins + 1.) as usize];

	for element in image.iter_element_values() {
		for pixel in element.data.iter() {
			let r = pixel.r() * GRID;
			let g = pixel.g() * GRID;
			let b = pixel.b() * GRID;

			let bin = (r * GRID + g * GRID + b * GRID) as usize;

			histogram[bin] += 1;
			color_bins[bin].push(pixel.to_gamma_srgb_channels());
		}
	}

	let shorted = histogram.iter().enumerate().filter(|&(_, &count)| count > 0).map(|(i, _)| i).collect::<Vec<usize>>();

	shorted
		.iter()
		.take(count as usize)
		.flat_map(|&i| {
			let list = &color_bins[i];

			let [mut r, mut g, mut b, mut a] = [0.; 4];

			for &[cr, cg, cb, ca] in list.iter() {
				r += cr;
				g += cg;
				b += cb;
				a += ca;
			}

			let len = list.len() as f32;
			let [r, g, b, a] = [r / len, g / len, b / len, a / len];

			// Reject NaN/out-of-range averages, then lift the gamma-space bin centroid to linear-light
			let in_gamut = a <= 1. && ![r, g, b, a].iter().any(|c| c.is_sign_negative() || !c.is_finite());
			in_gamut.then(|| Color::from_gamma_srgb_channels(r, g, b, a)).map(Item::new_from_element).into_iter()
		})
		.collect()
}

#[cfg(test)]
mod test {
	use super::*;
	use raster_types::Image;
	use raster_types::Raster;

	#[test]
	fn test_image_color_palette() {
		let result = image_color_palette(
			(),
			List::new_from_element(Raster::new_cpu(Image {
				width: 100,
				height: 100,
				data: vec![Color::from_rgbaf32(0., 0., 0., 1.).unwrap(); 10000],
				base64_string: None,
			})),
			1,
		);
		assert_eq!(futures::executor::block_on(result), List::new_from_element(Color::from_rgbaf32(0., 0., 0., 1.).unwrap()));
	}
}
