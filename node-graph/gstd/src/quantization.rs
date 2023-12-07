use autoquant::packing::ErrorFunction;
use graphene_core::quantization::*;
use graphene_core::raster::{Color, ImageFrame};
use graphene_core::Node;

/// The `GenerateQuantizationNode` encodes the brightness of each channel of the image as an integer number
/// signified by the samples parameter. This node is used to asses the loss of visual information when
/// quantizing the image using different fit functions.
pub struct GenerateQuantizationNode<N, M> {
	samples: N,
	function: M,
}

#[node_macro::node_fn(GenerateQuantizationNode)]
fn generate_quantization_fn(image_frame: ImageFrame<Color>, samples: u32, function: u32) -> [Quantization; 4] {
	generate_quantization_from_image_frame(&image_frame)
}

pub fn generate_quantization_from_image_frame(image_frame: &ImageFrame<Color>) -> [Quantization; 4] {
	let image = &image_frame.image;

	let len = image.data.len().min(10000);
	let data = image
		.data
		.iter()
		.enumerate()
		.filter(|(i, _)| i % (image.data.len() / len) == 0)
		.flat_map(|(_, x)| vec![x.r() as f64, x.g() as f64, x.b() as f64, x.a() as f64])
		.collect::<Vec<_>>();
	generate_quantization(data, len)
}
fn generate_quantization(data: Vec<f64>, samples: usize) -> [Quantization; 4] {
	let red = create_distribution(data.clone(), samples, 0);
	let green = create_distribution(data.clone(), samples, 1);
	let blue = create_distribution(data.clone(), samples, 2);
	let alpha = create_distribution(data, samples, 3);

	let fit_red = autoquant::calculate_error_function(&red, 1, &red);
	let fit_green = autoquant::calculate_error_function(&green, 1, &green);
	let fit_blue = autoquant::calculate_error_function(&blue, 1, &blue);
	let fit_alpha = autoquant::calculate_error_function(&alpha, 1, &alpha);
	let red_error: ErrorFunction<10> = autoquant::packing::ErrorFunction::new(fit_red.as_slice());
	let green_error: ErrorFunction<10> = autoquant::packing::ErrorFunction::new(fit_green.as_slice());
	let blue_error: ErrorFunction<10> = autoquant::packing::ErrorFunction::new(fit_blue.as_slice());
	let alpha_error: ErrorFunction<10> = autoquant::packing::ErrorFunction::new(fit_alpha.as_slice());
	let merged: ErrorFunction<20> = autoquant::packing::merge_error_functions(&red_error, &green_error);
	let merged: ErrorFunction<30> = autoquant::packing::merge_error_functions(&merged, &blue_error);
	let merged: ErrorFunction<40> = autoquant::packing::merge_error_functions(&merged, &alpha_error);

	let bin_size = 8;
	let mut distributions = [red, green, blue, alpha].into_iter();

	let bits = &merged.bits[bin_size];

	core::array::from_fn(|i| {
		let fit = autoquant::models::OptimizedLin::new(distributions.next().unwrap(), (1 << bits[i]) - 1);
		let parameters = fit.parameters();
		Quantization::new(parameters[0] as f32, parameters[1] as f32, bits[i] as u32)
	})
}

/*
// TODO: make this work with generic size parameters
fn generate_quantization<const N: usize>(data: Vec<f64>, samples: usize, channels: usize) -> [Quantization; N] {
	let mut quantizations = Vec::new();
	let mut merged_error: Option<ErrorFunction<10>> = None;
	let bin_size = 32;

	for i in 0..channels {
		let channel_data = create_distribution(data.clone(), samples, i);

		let fit = autoquant::calculate_error_function(&channel_data, 0, &channel_data);
		let error: ErrorFunction<10> = autoquant::packing::ErrorFunction::new(fit.as_slice());

		// Merge current error function with previous ones
		merged_error = match merged_error {
			Some(prev_error) => Some(autoquant::packing::merge_error_functions(&prev_error, &error)),
			None => Some(error.clone()),
		};

		println!("Merged: {merged_error:?}");

		let bits = merged_error.as_ref().unwrap().bits.iter().map(|x| x[i]).collect::<Vec<_>>();
		let model_fit = autoquant::models::OptimizedLin::new(channel_data, 1 << bits[bin_size]);
		let parameters = model_fit.parameters();
		let quantization = Quantization::new(parameters[0] as f32, parameters[1] as u32, bits[bin_size] as u32);

		quantizations.push(quantization);
	}

	core::array::from_fn(|x| quantizations[x])
}*/

fn create_distribution(data: Vec<f64>, samples: usize, channel: usize) -> Vec<(f64, f64)> {
	let data: Vec<f64> = data.chunks(4 * (data.len() / (4 * samples.min(data.len() / 4)))).map(|x| x[channel] as f64).collect();
	let max = *data.iter().max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)).unwrap();
	let data: Vec<f64> = data.iter().map(|x| x / max).collect();
	dbg!(max);
	//let data = autoquant::generate_normal_distribution(3.0, 1.1, 1000);
	//data.iter_mut().for_each(|x| *x = x.abs());
	let mut dist = autoquant::integrate_distribution(data);
	autoquant::drop_duplicates(&mut dist);
	let dist = autoquant::normalize_distribution(dist.as_slice());
	dist
}
