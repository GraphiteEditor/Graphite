use dyn_any::{DynAny, StaticType};
use graphene_core::quantization::*;
use graphene_core::raster::{Color, ImageFrame};
use graphene_core::Node;

/// The `GenerateQuantizationNode` encodes the brightness of each channel of the image as an integer number
/// sepified by the samples parameter. This node is used to asses the loss of visual information when
/// quantizing the image using different fit functions.
pub struct GenerateQuantizationNode<N, M> {
	samples: N,
	function: M,
}

#[node_macro::node_fn(GenerateQuantizationNode)]
fn generate_quantization_fn(image_frame: ImageFrame, samples: u32, function: u32) -> [Quantization; 4] {
	let image = image_frame.image;

	let len = image.data.len().min(10000);
	let mut channels: Vec<_> = (0..4).map(|_| Vec::with_capacity(image.data.len())).collect();
	image
		.data
		.iter()
		.enumerate()
		.filter(|(i, _)| i % (image.data.len() / len) == 0)
		.map(|(_, x)| vec![x.r() as f64, x.g() as f64, x.b() as f64, x.a() as f64])
		.for_each(|x| x.into_iter().enumerate().for_each(|(i, value)| channels[i].push(value)));
	log::info!("Quantizing {} samples", channels[0].len());
	log::info!("In {} channels", channels.len());
	let quantization: Vec<Quantization> = channels.into_iter().map(|x| generate_quantization_per_channel(x, samples)).collect();
	core::array::from_fn(|i| quantization[i].clone())
}

fn generate_quantization_per_channel(data: Vec<f64>, samples: u32) -> Quantization {
	let mut dist = autoquant::integrate_distribution(data);
	autoquant::drop_duplicates(&mut dist);
	let dist = autoquant::normalize_distribution(dist.as_slice());
	let max = dist.last().unwrap().0;
	/*let linear = Box::new(autoquant::SimpleFitFn {
		function: move |x| x / max,
		inverse: move |x| x * max,
		name: "identity",
	});*/

	let linear = Quantization {
		fn_index: 0,
		a: max as f32,
		b: 0.,
		c: 0.,
		d: 0.,
	};
	let log_fit = autoquant::models::OptimizedLog::new(dist, samples as u64);
	let parameters = log_fit.parameters();
	let log_fit = Quantization {
		fn_index: 1,
		a: parameters[0] as f32,
		b: parameters[1] as f32,
		c: parameters[2] as f32,
		d: parameters[3] as f32,
	};
	log_fit
}
