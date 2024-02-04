use graphene_core::raster::ImageFrame;
use graphene_core::Color;
use graphene_core::Node;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageColorPaletteNode<MaxSize> {
	max_size: MaxSize,
}

#[node_macro::node_fn(ImageColorPaletteNode)]
fn image_color_palette(frame: ImageFrame<Color>, max_size: u32) -> Vec<Color> {
	const GRID: f32 = 3.0;

	let bins = GRID * GRID * GRID;

	let mut histogram: Vec<usize> = vec![0; (bins + 1.0) as usize];
	let mut colors: Vec<Vec<Color>> = vec![vec![]; (bins + 1.0) as usize];

	for pixel in frame.image.data.iter() {
		let r = pixel.r() * GRID;
		let g = pixel.g() * GRID;
		let b = pixel.b() * GRID;

		let bin = (r * GRID + g * GRID + b * GRID) as usize;

		histogram[bin] += 1;
		colors[bin].push(pixel.to_gamma_srgb());
	}

	let shorted = histogram.iter().enumerate().filter(|(_, &count)| count > 0).map(|(i, _)| i).collect::<Vec<usize>>();

	let mut palette = vec![];

	for i in shorted.iter().take(max_size as usize) {
		let list = colors[*i].clone();

		let mut r = 0.0;
		let mut g = 0.0;
		let mut b = 0.0;
		let mut a = 0.0;

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

	return palette;
}

#[cfg(test)]
mod test {
	use graphene_core::{raster::Image, value::CopiedNode};

	use super::*;

	#[test]
	fn test_image_color_palette() {
		assert_eq!(
			ImageColorPaletteNode { max_size: CopiedNode(1u32) }.eval(ImageFrame {
				image: Image {
					width: 100,
					height: 100,
					data: vec![Color::from_rgbaf32(0.0, 0.0, 0.0, 1.0).unwrap(); 10000],
					base64_string: None,
				},
				..Default::default()
			}),
			[Color::from_rgbaf32(0.0, 0.0, 0.0, 1.0).unwrap()]
		);
	}
}
