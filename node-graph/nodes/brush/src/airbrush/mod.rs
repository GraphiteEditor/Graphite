mod consts;
mod convert;
mod kernel;
mod pipeline;
mod region;
mod render;
mod stroke;

use brush_types::BrushCache;
use core_types::list::{ATTR_COLOR, ATTR_DIAMETER, ATTR_FLOW, ATTR_HARDNESS, Item, List};
use core_types::{ATTR_TRANSFORM, Ctx, ExtractFootprint};
use graphic_types::Graphic;
use pipeline::{AirbrushPipeline, AirbrushPipelineArgs};
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
		let color = item.attribute_cloned_or(ATTR_COLOR, crate::DEFAULT_COLOR);
		let diameter = item.attribute_cloned_or(ATTR_DIAMETER, crate::DEFAULT_DIAMETER);
		let hardness = item.attribute_cloned_or(ATTR_HARDNESS, crate::DEFAULT_HARDNESS / 100.);
		let flow = item.attribute_cloned_or(ATTR_FLOW, crate::DEFAULT_FLOW / 100.);
		match item.into_element() {
			Graphic::Stroke(list) => strokes.extend(
				list.into_iter()
					.map(Item::into_element)
					.filter(|stroke| !stroke.is_empty() && stroke.is_valid())
					.map(|stroke| stroke::StyledStroke {
						color,
						diameter,
						hardness,
						flow,
						stroke,
					}),
			),
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

#[node_macro::node(category(""), inject_scope)]
async fn airbrush_pipeline<'a: 'n>(
	_ctx: impl Ctx,
	#[scope(ProtoNodeIdentifier::new("graphene_std::platform_application_io::WgpuExecutorNode"))] executor: &'a WgpuExecutor,
	#[data] pipeline: WgpuPipelineCache,
) -> WgpuPipelineCache {
	executor.pipeline_init::<AirbrushPipeline>(pipeline);
	pipeline.clone()
}
