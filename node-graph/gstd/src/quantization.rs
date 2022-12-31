use graphene_core::raster::{Color, Image};
use graphene_core::Node;

pub struct GenerateQuantizationNode<N: Node<(), Output = u32>, M: Node<(), Output = u32>> {
	samples: N,
	function: M,
}

#[node_macro::node_fn(GenerateQuantizationNode)]
fn generate_quantization_fn(image: Image, samples: u32, function: u32) -> Image {
	// Scale the input image, this can be removed by adding an extra parameter to the fit function.
	let max_energy = 16380.;
	let data: Vec<f64> = image.data.iter().flat_map(|x| vec![x.r() as f64, x.g() as f64, x.b() as f64]).collect();
	let data: Vec<f64> = data.iter().map(|x| x * max_energy).collect();
	let mut dist = autoquant::integrate_distribution(data);
	autoquant::drop_duplicates(&mut dist);
	let dist = autoquant::normalize_distribution(dist.as_slice());
	let max = dist.last().unwrap().0;
	let linear = Box::new(autoquant::SimpleFitFn {
		function: move |x| x / max,
		inverse: move |x| x * max,
		name: "identity",
	});
	let best = match function {
		0 => linear as Box<dyn autoquant::FitFn>,
		1 => linear as Box<dyn autoquant::FitFn>,
		2 => Box::new(autoquant::models::OptimizedLog::new(dist, 20)) as Box<dyn autoquant::FitFn>,
		_ => linear as Box<dyn autoquant::FitFn>,
	};

	let roundtrip = |sample: f32| -> f32 {
		let encoded = autoquant::encode(sample as f64 * max_energy, best.as_ref(), samples);
		let decoded = autoquant::decode(encoded, best.as_ref(), samples) / max_energy;
		log::trace!("{} enc: {} dec: {}", sample, encoded, decoded);
		decoded as f32
	};

	let new_data = image
		.data
		.iter()
		.map(|c| {
			let r = roundtrip(c.r());
			let g = roundtrip(c.g());
			let b = roundtrip(c.b());
			let a = c.a();

			Color::from_rgbaf32_unchecked(r, g, b, a)
		})
		.collect();
	Image { data: new_data, ..image }
}
