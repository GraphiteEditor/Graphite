use crate::raster_kernel::{FILTERABLE_FORMAT, RasterKernel, ToGpuParams, Wgsl, raster_kernel_pipeline, run_raster_kernel, run_raster_kernel_with_format};
use crate::{gpu_param_enum, gpu_params};
use core_types::Interrupt;
use core_types::context::Ctx;
use core_types::list::Item;
use glam::DAffine2;
use num_enum::IntoPrimitive;
use raster_types::{GPU, Raster, Storage};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelInput;

/// Blurs the image with a Gaussian or box blur kernel filter.
#[node_macro::node(category("Raster: Filter"), icon("NodeBlur"))]
fn blur(
	_: impl Ctx,
	/// The image to be blurred.
	image_frame: Item<Raster<GPU>>,
	/// The radius of the blur kernel.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	radius: Item<f64>,
	/// Use a lower-quality box kernel instead of a circular Gaussian kernel. This is faster but produces boxy artifacts.
	box_blur: Item<bool>,
	/// Opt to incorrectly apply the filter with color calculations in gamma space for compatibility with the results from other software.
	gamma: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	let (radius, box_blur, gamma) = (*radius.element(), *box_blur.element(), *gamma.element());
	let pipeline = pipeline.element();

	// TODO: Scale the radius through the footprint so it acts as the document-space length its Pixels unit promises, with a
	// window that varies continuously rather than snapping to whole source pixels
	if radius < 0.1 || image_frame.element().is_empty() {
		return Ok(image_frame);
	}

	// Each pass sums premultiplied colors into a `FILTERABLE_FORMAT` intermediate, and a kernel reaching past `MAX_BLUR_REACH` runs on the image halved
	// until it fits, weighing each halved texel by how much of the image it covers, then is scaled back up bilinearly
	let (raster, attributes) = image_frame.into_parts();
	let space = if gamma { BlurSpace::Gamma } else { BlurSpace::Linear };
	let size = raster.size();
	let full_size = size.as_ivec2().to_array();

	let mut levels = 0;
	while blur_extent(radius, box_blur, 1 << levels).1 > MAX_BLUR_REACH {
		levels += 1;
	}
	let level_size = |level: u32| size.map(|side| side.div_ceil(1 << level));

	let mut source = raster;
	for level in 1..=levels {
		let kernel = BlurHalve {
			space: if level == 1 { space } else { BlurSpace::Premultiplied },
			scale: 1 << (level - 1),
			full_size,
		};
		source = run_raster_kernel_with_format(&kernel, blur_input(&source), level_size(level), FILTERABLE_FORMAT, pipeline)?;
	}

	let (radius, reach) = blur_extent(radius, box_blur, 1 << levels);
	let pass = BlurPass {
		radius,
		reach,
		box_blur,
		space: if levels == 0 { space } else { BlurSpace::Premultiplied },
		scale: 1 << levels,
		full_size,
	};
	let horizontal = run_raster_kernel_with_format(&BlurHorizontal(pass.clone()), blur_input(&source), level_size(levels), FILTERABLE_FORMAT, pipeline)?;
	if levels == 0 {
		let blurred = run_raster_kernel(&BlurVertical(pass), blur_input(&horizontal), size, pipeline)?;
		return Ok(Item::from_parts(blurred, attributes));
	}
	let vertical = run_raster_kernel_with_format(&BlurVertical(pass), blur_input(&horizontal), level_size(levels), FILTERABLE_FORMAT, pipeline)?;

	let upsample = BlurUpsample { scale: (1 << levels) as f32, space };
	let blurred = run_raster_kernel(&upsample, blur_input(&vertical), size, pipeline)?;
	Ok(Item::from_parts(blurred, attributes))
}

/// Applies a median filter to reduce noise while preserving edges.
#[node_macro::node(category("Raster: Filter"), icon("NodeMedianFilter"), memoize)]
fn median_filter(
	_: impl Ctx,
	/// The image to be filtered.
	image_frame: Item<Raster<GPU>>,
	/// The radius of the filter kernel. Larger values remove more noise but may blur fine details.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..50)]
	radius: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	let radius = *radius.element();

	// TODO: Scale the radius through the footprint so it acts as the document-space length its Pixels unit promises, with a
	// window that varies continuously rather than snapping to whole source pixels
	if radius < 0.5 || image_frame.element().is_empty() {
		return Ok(image_frame);
	}

	let (image, attributes) = image_frame.into_parts();
	let size = image.size();
	let kernel = MedianWindow {
		radius: (radius as u32).min(size.max_element()),
	};
	let input = RasterKernelInput {
		raster: &image,
		output_to_texel: DAffine2::IDENTITY,
	};
	let filtered_image = run_raster_kernel(&kernel, vec![input], size, pipeline.element())?;

	Ok(Item::from_parts(filtered_image, attributes))
}

gpu_params! {
	#[derive(Clone)]
	struct MedianWindow {
		radius: u32,
	}
}

impl RasterKernel for MedianWindow {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(include_str!("wgsl/median_filter.wgsl"))
	}
}

/// The farthest texel from the center a blur pass weighs, beyond which the image is first halved until the kernel fits.
const MAX_BLUR_REACH: i32 = 64;

/// The radius of a blur over texels `scale` image pixels wide, and the farthest texel from the center it weighs.
///
/// A box covers its whole texels and a fraction of the texel at each end, which keeps the window as wide as the whole pixel window of the unhalved image once halved,
/// and a Gaussian leaves out the blur that halving and doubling the image add.
fn blur_extent(radius: f64, box_blur: bool, scale: u32) -> (f32, i32) {
	let scale = scale as f64;
	if box_blur {
		let radius = ((radius as i32) as f64 + 0.5) / scale - 0.5;
		(radius as f32, radius.ceil() as i32)
	} else {
		let radius = (radius * radius - (scale * scale - 1.) / 3.).max(0.).sqrt() / scale;
		(radius as f32, (3. * radius).ceil() as i32)
	}
}

fn blur_input(raster: &Raster<GPU>) -> Vec<RasterKernelInput<'_>> {
	vec![RasterKernelInput {
		raster,
		output_to_texel: DAffine2::IDENTITY,
	}]
}

/// The colors a blur pass reads from its input or writes to its output, around the premultiplied colors it sums.
#[derive(Clone, Copy, IntoPrimitive)]
#[repr(u32)]
enum BlurSpace {
	Premultiplied,
	Linear,
	Gamma,
}

gpu_param_enum!(u32: BlurSpace);

gpu_params! {
	/// Averages each 2x2 block of texels `scale` image pixels wide, weighed by how much of the `full_size` image each covers.
	#[derive(Clone)]
	struct BlurHalve {
		space: BlurSpace,
		scale: i32,
		full_size: [i32; 2],
	}
}

impl RasterKernel for BlurHalve {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_halve.wgsl")))
	}
}

gpu_params! {
	/// One axis of a blur over texels `scale` image pixels wide, each weighed by how much of the `full_size` image it covers.
	#[derive(Clone)]
	struct BlurPass {
		radius: f32,
		reach: i32,
		box_blur: bool,
		space: BlurSpace,
		scale: i32,
		full_size: [i32; 2],
	}
}

struct BlurHorizontal(BlurPass);

impl ToGpuParams for BlurHorizontal {
	type Params = BlurPass;
	fn to_params(&self) -> BlurPass {
		self.0.clone()
	}
}

impl RasterKernel for BlurHorizontal {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_horizontal.wgsl")))
	}
}

/// Reads its input in pairs of texels sampled bilinearly between them, so the input must be in [`FILTERABLE_FORMAT`].
struct BlurVertical(BlurPass);

impl ToGpuParams for BlurVertical {
	type Params = BlurPass;
	fn to_params(&self) -> BlurPass {
		self.0.clone()
	}
}

impl RasterKernel for BlurVertical {
	const INPUTS: usize = 1;
	const FILTERED: bool = true;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_vertical.wgsl")))
	}
}

gpu_params! {
	#[derive(Clone)]
	struct BlurUpsample {
		scale: f32,
		space: BlurSpace,
	}
}

impl RasterKernel for BlurUpsample {
	const INPUTS: usize = 1;
	const FILTERED: bool = true;
	fn wgsl() -> Wgsl {
		Wgsl::function(concat!(include_str!("wgsl/blur.wgsl"), include_str!("wgsl/blur_upsample.wgsl")))
	}
}
