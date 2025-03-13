use graphene_core::raster::image::ImageFrameTable;
use graphene_core::{Color, Ctx};

#[node_macro::node(category("Raster"))]
async fn image_color_palette(
	_: impl Ctx,
	image: ImageFrameTable<Color>,
	#[min(1.)]
	#[max(28.)]
	max_size: u32,
) -> Vec<Color> {
	const GRID: f32 = 3.;

	let bins = GRID * GRID * GRID;

	let mut histogram: Vec<usize> = vec![0; (bins + 1.) as usize];
	let mut colors: Vec<Vec<Color>> = vec![vec![]; (bins + 1.) as usize];

	let image = image.one_instance().instance;

	for pixel in image.data.iter() {
		let r = pixel.r() * GRID;
		let g = pixel.g() * GRID;
		let b = pixel.b() * GRID;

		let bin = (r * GRID + g * GRID + b * GRID) as usize;

		histogram[bin] += 1;
		colors[bin].push(pixel.to_gamma_srgb());
	}

	let shorted = histogram.iter().enumerate().filter(|&(_, &count)| count > 0).map(|(i, _)| i).collect::<Vec<usize>>();

	let mut palette = vec![];

	for i in shorted.iter().take(max_size as usize) {
		let list = colors[*i].clone();

		let mut r = 0.;
		let mut g = 0.;
		let mut b = 0.;
		let mut a = 0.;

		for color in list.iter() {
			r += color.r();
			g += color.g();
			b += color.b();
			a += color.a();
		}

		r /= list.len() as f32;
		g /= list.len() as f32;
		b /= list.len() as f32;
		a /= list.len() as f32;

		let color = Color::from_rgbaf32(r, g, b, a).unwrap();

		palette.push(color);
	}

	palette
}

#[cfg(test)]
mod test {
	use super::*;
	use graphene_core::raster::image::{Image, ImageFrameTable};

	#[test]
	fn test_image_color_palette() {
		let result = image_color_palette(
			(),
			ImageFrameTable::new(Image {
				width: 100,
				height: 100,
				data: vec![Color::from_rgbaf32(0., 0., 0., 1.).unwrap(); 10000],
				base64_string: None,
			}),
			1,
		);
		assert_eq!(futures::executor::block_on(result), [Color::from_rgbaf32(0., 0., 0., 1.).unwrap()]);
	}
}
