pub use raster_kernels::{GpuParamField, GpuParams, Lut1d, Lut3d, MapColor, RasterKernel, RasterKernelLookup, ToGpuParams, Wgsl};
pub use wgpu_executor::raster_kernel::{FILTERABLE_FORMAT, KernelFrame, MAX_FRAME_SIDE, map_raster, raster_placement, run_raster_kernel, run_raster_kernel_with_format, union_frame};

use core_types::Ctx;
use core_types::list::Item;
use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelRuntime;
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

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
