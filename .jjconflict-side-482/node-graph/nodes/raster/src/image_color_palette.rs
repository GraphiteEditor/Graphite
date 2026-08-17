use core_types::color::Color;
use core_types::context::{Ctx, ExtractIndex, InjectIndex};
use core_types::gpoll::{GraphError, Interrupt};
use raster_types::{CPU, Raster};

#[node_macro::node(category("Color"))]
fn image_color_palette(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	image: IList<Raster<CPU>>,
	#[default(4)]
	#[hard(1..)]
	count: u32,
) -> Result<IList<Color>, Interrupt> {
	const GRID: f32 = 3.;

	let bins = GRID * GRID * GRID;

	let mut histogram = vec![0; (bins + 1.) as usize];
	// Each bin stores `(red, green, blue, alpha)` tuples in sRGB gamma space; averaging in gamma space gives perceptually-uniform binning.
	let mut color_bins: Vec<Vec<[f32; 4]>> = vec![Vec::new(); (bins + 1.) as usize];

	for row in 0..image.len() {
		let element = image.element_ref(row);
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

	let palette: Vec<Color> = shorted
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
			in_gamut.then(|| Color::from_gamma_srgb_channels(r, g, b, a)).into_iter()
		})
		.collect();

	palette.get(ctx.index() as usize).copied().ok_or_else(|| GraphError::past_end().into())
}

#[cfg(test)]
mod test {
	use super::*;
	use raster_types::Image;
	use raster_types::Raster;

	#[test]
	fn test_image_color_palette() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = core_types::arena::Arena::new(1 << 22).unwrap();
		let generations = [];
		let scope = core_types::context::EvalScope::new(None, None, None, &generations, &arena);
		let ctx = core_types::context::ContextImpl::root(&scope);

		let raster = Raster::new_cpu(Image {
			width: 100,
			height: 100,
			data: vec![Color::from_rgbaf32(0., 0., 0., 1.).unwrap(); 10000],
			base64_string: None,
		});
		let source = core_types::value::LeveledValueSource::new(vec![raster]);
		let core_types::record::LevelStatus::Batch(batch, _) = core_types::record::materialize_level(&source, &ctx, &arena, &frames) else {
			panic!("materialize failed")
		};
		let image = unsafe { core_types::node::List::<Raster<CPU>>::new(batch) };

		// The root context addresses lane 0, the palette's first color
		let color = image_color_palette(&ctx, image, 1).unwrap();
		assert_eq!(color, Color::from_rgbaf32(0., 0., 0., 1.).unwrap());
	}
}
