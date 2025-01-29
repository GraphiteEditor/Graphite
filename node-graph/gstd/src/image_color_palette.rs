use graphene_core::raster::image::ImageFrameTable;
use graphene_core::transform::Footprint;
use graphene_core::{Color, Ctx};

#[node_macro::node(category("Raster"))]
async fn image_color_palette<F: 'n + Send>(
	#[implementations(
		(),
		Footprint,
	)]
	footprint: F,
	#[implementations(
		() -> ImageFrameTable<Color>,
		Footprint -> ImageFrameTable<Color>,
	)]
	image: impl Node<F, Output = ImageFrameTable<Color>>,
	#[min(1.)]
	#[max(28.)]
	max_size: u32,
) -> Vec<Color> {
	const GRID: f32 = 3.;

	let bins = GRID * GRID * GRID;

	let mut histogram: Vec<usize> = vec![0; (bins + 1.) as usize];
	let mut colors: Vec<Vec<Color>> = vec![vec![]; (bins + 1.) as usize];

	let image = image.eval(footprint).await;
	let image = image.one_item();

	for pixel in image.image.data.iter() {
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

	use graph_craft::generic::FnNode;
	use graphene_core::raster::image::{ImageFrame, ImageFrameTable};
	use graphene_core::raster::Image;
	use graphene_core::value::CopiedNode;
	use graphene_core::Node;

	#[test]
	fn test_image_color_palette() {
		let node = ImageColorPaletteNode {
			max_size: CopiedNode(1u32),
			image: FnNode::new(|_| {
				Box::pin(async move {
					ImageFrameTable::new(ImageFrame {
						image: Image {
							width: 100,
							height: 100,
							data: vec![Color::from_rgbaf32(0., 0., 0., 1.).unwrap(); 10000],
							base64_string: None,
						},
						..Default::default()
					})
				})
			}),
		};
		assert_eq!(futures::executor::block_on(node.eval(())), [Color::from_rgbaf32(0., 0., 0., 1.).unwrap()]);
	}
}
