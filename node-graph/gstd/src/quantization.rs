use graphene_core::raster::{Color, Image};
use graphene_core::Node;

pub struct GenerateQuantizationNode<N: Node<(), Output = u32>>(N);

impl<N: Node<(), Output = u32>> Node<Image> for GenerateQuantizationNode<N> {
	type Output = Image;

	fn eval(self, input: Image) -> Self::Output {
		let samples = self.0.eval(());
		generate_quantization_fn(samples, input)
	}
}

impl<N: Node<(), Output = u32> + Copy> Node<Image> for &GenerateQuantizationNode<N> {
	type Output = Image;

	fn eval(self, input: Image) -> Self::Output {
		let samples = self.0.eval(());
		generate_quantization_fn(samples, input)
	}
}

impl<N: Node<(), Output = u32>> GenerateQuantizationNode<N> {
	pub fn new(node: N) -> Self {
		Self(node)
	}
}

fn generate_quantization_fn(samples: u32, input: Image) -> Image {
	let data: Vec<f64> = input
		.data
		.iter()
		.map(|x| vec![x.r() as f64, x.g() as f64, x.b() as f64])
		.reduce(|mut acc, x| {
			acc.extend_from_slice(&x);
			acc
		})
		.unwrap_or_default();
	let mut dist = autoquant::integrate_distribution(data.clone());
	autoquant::drop_duplicates(&mut dist);
	let dist = autoquant::normalize_distribution(dist.as_slice());
	let max = dist.last().unwrap().0;
	let best = Box::new(autoquant::SimpleFitFn {
		function: move |x| x / max,
		inverse: move |x| x * max,
		name: "identity",
	});

	//let functions = autoquant::fit_functions(dist);
	let best = (0., best);
	/*
			.into_iter()
			.map(|f| (autoquant::calculate_sampled_error(&data, f.as_ref(), samples), f))
			.min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
			.unwrap();
	*/
	let roundtrip = |sample: f32| -> f32 {
		let encoded = autoquant::encode(sample as f64, best.1.as_ref(), samples);
		let decoded = autoquant::decode(encoded, best.1.as_ref(), samples);
		decoded as f32
	};

	let new_data = input
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
	Image { data: new_data, ..input }
}
