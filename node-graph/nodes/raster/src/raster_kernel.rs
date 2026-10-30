pub use raster_kernels::{GpuParamField, GpuParams, Lut1d, Lut3d, MapColor, RasterKernel, RasterKernelLookup, ToGpuParams, Wgsl};
pub use wgpu_executor::raster_kernel::{FILTERABLE_FORMAT, KernelFrame, MAX_FRAME_SIDE, map_raster, raster_placement, run_raster_kernel, run_raster_kernel_with_format, union_frame};

use core_types::color::Color;
use core_types::list::Item;
use core_types::{Ctx, NodeError};
use graphic_types::Adjustment;
use raster_kernels::adjustments::with_color_map;
use raster_types::{GPU, Raster};
use vector_types::Gradient;
use wgpu_executor::raster_kernel::adjust_raster;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelRuntime;
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

/// Content an [`Adjustment`] can be carried out on, a raster with its kernel on the GPU and a color or gradient with the kernel's mapping of each color.
pub trait Adjust: Sized {
	fn adjust(self, adjustment: &Adjustment, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError>;
}

impl Adjust for Color {
	fn adjust(self, adjustment: &Adjustment, _: &WgpuPipelineCache) -> Result<Self, NodeError> {
		Ok(with_color_map(adjustment, |map| map(self)))
	}
}

impl Adjust for Gradient {
	fn adjust(self, adjustment: &Adjustment, _: &WgpuPipelineCache) -> Result<Self, NodeError> {
		Ok(with_color_map(adjustment, |map| self.map_colors(|color| map(*color))))
	}
}

impl Adjust for Raster<GPU> {
	fn adjust(self, adjustment: &Adjustment, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError> {
		adjust_raster(adjustment, self, pipeline)
	}
}

impl<T: Adjust> Adjust for Item<T> {
	fn adjust(self, adjustment: &Adjustment, pipeline: &WgpuPipelineCache) -> Result<Self, NodeError> {
		let (element, attributes) = self.into_parts();
		Ok(Item::from_parts(element.adjust(adjustment, pipeline)?, attributes))
	}
}

#[node_macro::node(category(""), inject_scope)]
fn raster_kernel_pipeline(
	_: impl Ctx,
	#[scope(ProtoNodeIdentifier::new("graphene_std::platform_application_io::TryWgpuExecutorNode"))] executor: Item<Option<WgpuExecutor>>,
	#[data] pipeline: WgpuPipelineCache,
) -> Item<WgpuPipelineCache> {
	if let Some(executor) = executor.into_element() {
		executor.pipeline_init::<RasterKernelRuntime>(pipeline);
	}
	Item::new_from_element(pipeline.clone())
}
