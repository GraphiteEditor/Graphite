use crate::gpu_params;
use crate::raster_kernel::{RasterKernel, Wgsl, raster_kernel_pipeline, run_raster_kernel};
use core_types::NodeError;
use core_types::color::Color;
use core_types::context::Ctx;
use core_types::list::Item;
use core_types::ops::ConvertAsync;
use core_types::transform::Footprint;
use glam::DAffine2;
use raster_types::{CPU, GPU, Raster, Storage};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelInput;

// Dehazing algorithm based on "Single Image Haze Removal Using Dark Channel Prior"
// Paper: <https://www.researchgate.net/publication/220182411_Single_Image_Haze_Removal_Using_Dark_Channel_Prior>
// TODO: Make this algorithm work with negative strength values
#[node_macro::node(category("Raster: Filter"), icon("NodeDehaze"), placeholder = pending_dehaze)]
async fn dehaze(
	_: impl Ctx,
	image_frame: Item<Raster<GPU>>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	strength: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, NodeError> {
	let strength = *strength.element();
	let pipeline = pipeline.element();
	if image_frame.element().is_empty() {
		return Ok(image_frame);
	}

	let executor = pipeline.executor().cloned().ok_or_else(|| NodeError::new("A GPU is required to process a GPU raster"))?;
	let (raster, attributes) = image_frame.into_parts();
	let size = raster.size();

	// TODO: Break out this pair of steps into its own node, with a Memoize node which caches the pair of outputs, so the strength can be adjusted without recomputing these two steps.
	let radius = (PATCH_SIZE / 2) as i32;
	let eroded = run_raster_kernel(&DehazeErode { radius, vertical: false }, vec![dehaze_input(&raster)], size, pipeline)?;
	let dark_channel = run_raster_kernel(&DehazeErode { radius, vertical: true }, vec![dehaze_input(&eroded)], size, pipeline)?;
	let atmospheric_light = estimate_atmospheric_light(&dark_channel.clone().convert(Footprint::DEFAULT, executor).await);

	// The guided filter's intermediates stay in the executor's raster format rather than `FILTERABLE_FORMAT` since its variances lose their precision at half-float
	let radius = RADIUS as i32;
	let guide = DehazeGuideHorizontal {
		radius,
		strength: (strength / 100.) as f32,
	};
	let guide = run_raster_kernel(&guide, vec![dehaze_input(&dark_channel)], size, pipeline)?;
	let coefficients = DehazeGuideVertical { radius, epsilon: EPSILON as f32 };
	let coefficients = run_raster_kernel(&coefficients, vec![dehaze_input(&guide)], size, pipeline)?;
	let coefficients = run_raster_kernel(&DehazeCoefficientsHorizontal { radius }, vec![dehaze_input(&coefficients)], size, pipeline)?;
	let recover = DehazeRecover {
		radius,
		minimum_transmission: TX,
		atmospheric_light,
	};
	let dehazed = run_raster_kernel(&recover, vec![dehaze_input(&coefficients), dehaze_input(&raster)], size, pipeline)?;

	Ok(Item::from_parts(dehazed.with_coverage(raster.coverage()), attributes))
}

fn pending_dehaze(image_frame: &Item<Raster<GPU>>, _strength: &Item<f64>, _pipeline: &Item<WgpuPipelineCache>) -> Item<Raster<GPU>> {
	image_frame.clone()
}

fn dehaze_input(raster: &Raster<GPU>) -> RasterKernelInput<'_> {
	RasterKernelInput {
		raster,
		output_to_texel: DAffine2::IDENTITY,
	}
}

/// The average color of the pixels with the brightest dark channel, from each pixel's color with its dark channel as alpha.
fn estimate_atmospheric_light(dark_channel: &Raster<CPU>) -> Color {
	let pixels = &dark_channel.data;
	let num_pixels = ((TOP_PERCENT / 100.) * pixels.len() as f64).ceil() as usize;

	let mut order: Vec<usize> = (0..pixels.len()).collect();
	order.select_nth_unstable_by(num_pixels - 1, |&a, &b| pixels[b].a().total_cmp(&pixels[a].a()).then(a.cmp(&b)));

	let mut atm_sum = [0., 0., 0.];
	for &index in &order[..num_pixels] {
		let pixel = pixels[index];
		atm_sum[0] += pixel.r() as f64;
		atm_sum[1] += pixel.g() as f64;
		atm_sum[2] += pixel.b() as f64;
	}

	let num_pixels = num_pixels as f64;
	Color::from_rgbaf32_unchecked((atm_sum[0] / num_pixels) as f32, (atm_sum[1] / num_pixels) as f32, (atm_sum[2] / num_pixels) as f32, 1.)
}

gpu_params! {
	/// One axis of the dark channel's erosion, the horizontal pass eroding each pixel's darkest channel and the vertical pass the horizontal pass's alpha,
	/// both writing the pixel's color clamped to 0 to 1 with the eroded value as alpha.
	#[derive(Clone)]
	struct DehazeErode {
		radius: i32,
		vertical: bool,
	}
}

impl RasterKernel for DehazeErode {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_erode.wgsl"))
	}
}

gpu_params! {
	/// Horizontal means of the guidance luma, the transmission, the luma squared, and the luma times the transmission.
	#[derive(Clone)]
	struct DehazeGuideHorizontal {
		radius: i32,
		strength: f32,
	}
}

impl RasterKernel for DehazeGuideHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_guide_horizontal.wgsl"))
	}
}

gpu_params! {
	/// Completes the guided filter's means vertically into its linear coefficients.
	#[derive(Clone)]
	struct DehazeGuideVertical {
		radius: i32,
		epsilon: f32,
	}
}

impl RasterKernel for DehazeGuideVertical {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_guide_vertical.wgsl"))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct DehazeCoefficientsHorizontal {
		radius: i32,
	}
}

impl RasterKernel for DehazeCoefficientsHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_coefficients_horizontal.wgsl"))
	}
}

gpu_params! {
	/// Completes the coefficients' means vertically into the refined transmission, recovering the image from its haze.
	#[derive(Clone)]
	struct DehazeRecover {
		radius: i32,
		minimum_transmission: f32,
		atmospheric_light: Color,
	}
}

impl RasterKernel for DehazeRecover {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/dehaze_recover.wgsl"))
	}
}

// There is no real point in modifying these values because they do not change the final result all that much.
// The authors of the paper recommended using these values to get a reasonable balance of performance and quality.
// TODO: The patch size and guided filter radius are whole source pixels, so the effect's scale follows the image's resolution.
// Derive them from the footprint so the dehaze acts at a document-space scale independent of how the image was sampled.
const PATCH_SIZE: u32 = 15;
const TOP_PERCENT: f64 = 0.001;
const RADIUS: u32 = 60;
const EPSILON: f64 = 0.0001;
const TX: f32 = 0.1;
