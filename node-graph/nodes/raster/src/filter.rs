use crate::gpu_params;
use crate::raster_kernel::{RasterKernel, Wgsl, raster_kernel_pipeline, run_raster_kernel};
use core_types::Interrupt;
use core_types::context::Ctx;
use core_types::list::Item;
use glam::DAffine2;
use raster_types::{GPU, Raster, Storage};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelInput;

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

	Ok(Item::from_parts(filtered_image.with_coverage(image.coverage()), attributes))
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
