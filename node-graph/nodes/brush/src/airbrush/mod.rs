mod convert;
mod helpers;
mod region;
mod stroke;

use brush_types::{BrushCache, BrushStyle};
use core_types::list::{ATTR_BRUSH_STYLE, Item, List};
use core_types::{ATTR_TRANSFORM, Ctx, ExtractFootprint, ProtoNodeIdentifier};
use graphic_types::Graphic;
use helpers::{AirbrushPipeline, AirbrushPipelineArgs};
use raster_types::{GPU, Raster};
use wgpu_executor::{WgpuExecutor, WgpuPipelineCache};

#[node_macro::node(category("Raster: Brush"))]
pub async fn airbrush<'a: 'n>(ctx: impl Ctx + ExtractFootprint, strokes: List<Graphic>, cache: BrushCache, #[scope(airbrush_pipeline::IDENTIFIER)] pipeline: WgpuPipelineCache) -> List<Raster<GPU>> {
	let mut stack = vec![strokes.into_iter()];
	let mut strokes = Vec::new();
	while let Some(top) = stack.last_mut() {
		let Some(item) = top.next() else {
			stack.pop();
			continue;
		};
		let style = item.attribute_cloned_or(ATTR_BRUSH_STYLE, BrushStyle::default());
		match item.into_element() {
			Graphic::Stroke(list) => strokes.extend(list.into_iter().map(Item::into_element).filter(|stroke| !stroke.is_empty()).map(|stroke| (style, stroke))),
			Graphic::Graphic(nested) => stack.push(nested.into_iter()),
			_ => {}
		}
	}

	let args = AirbrushPipelineArgs {
		footprint: *ctx.footprint(),
		strokes: &strokes,
		cache: &cache,
	};
	let Some((texture, transform)) = pipeline.run::<AirbrushPipeline>(&args).await else {
		return List::new();
	};
	let raster = Raster::<GPU>::new_gpu(texture);
	List::new_from_item(Item::new_from_element(raster).with_attribute(ATTR_TRANSFORM, transform))
}

const WGPU_EXECUTOR_IDENTIFIER: ProtoNodeIdentifier = ProtoNodeIdentifier::new("graphene_std::platform_application_io::WgpuExecutorNode");

#[node_macro::node(category(""), inject_scope)]
async fn airbrush_pipeline<'a: 'n>(_ctx: impl Ctx, #[scope(WGPU_EXECUTOR_IDENTIFIER)] executor: &'a WgpuExecutor, #[data] pipeline: WgpuPipelineCache) -> WgpuPipelineCache {
	executor.pipeline_init::<AirbrushPipeline>(pipeline);
	pipeline.clone()
}
