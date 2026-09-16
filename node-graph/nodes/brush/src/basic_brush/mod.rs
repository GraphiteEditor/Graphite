mod consts;
mod convert;
mod kernel;
mod pipeline;
mod region;
mod render;
mod stroke;

use brush_types::BrushCache;
use core_types::attribute::{Attr, BrushColor, Diameter, Flow, Hardness, Transform as TransformAttr};
use core_types::gpoll::Interrupt;
use core_types::{Ctx, ExtractFootprint, ProtoNodeIdentifier};

use graphic_types::Graphic;
use pipeline::{BasicBrushPipeline, BasicBrushPipelineArgs};
use raster_types::{GPU, Raster};
use wgpu_executor::WgpuPipelineCache;

#[node_macro::node(category("Raster: Brush"))]
pub fn basic_brush(
	ctx: impl Ctx + ExtractFootprint,
	strokes: IList<Graphic<'static>>,
	#[widget(ParsedWidgetOverride::Hidden)]
	#[data]
	cache: BrushCache,
	#[scope(basic_brush_pipeline::IDENTIFIER)] pipeline: WgpuPipelineCache,
) -> Result<(Raster<GPU>, Attr<'static, TransformAttr>), Interrupt> {
	// The style rides each lane, so a stroke is collected together with the columns of the lane holding it
	let mut styled = Vec::new();
	for index in 0..strokes.len() {
		let element = strokes.element_ref(index);
		let lane = strokes.lane(index);
		let style = stroke::StyleColumns {
			color: lane.attr::<BrushColor>(),
			diameter: lane.attr::<Diameter>(),
			hardness: lane.attr::<Hardness>(),
			flow: lane.attr::<Flow>(),
		};
		collect_styled_strokes(element, style, &mut styled);
	}

	let args = BasicBrushPipelineArgs {
		footprint: *ctx.footprint(),
		strokes: &styled,
		cache,
	};
	// Nothing to rasterize (no strokes, or a degenerate footprint) ends the level rather than
	// fabricating a texture, since a GPU raster has no empty form
	let Some((texture, transform)) = pipeline.run::<BasicBrushPipeline>(&args) else {
		return Err(core_types::gpoll::GraphError::past_end().into());
	};

	Ok((Raster::<GPU>::new_gpu(texture), Attr(transform)))
}

/// Walks a graphic for the brush strokes beneath it, carrying the style of the lane each one came from.
fn collect_styled_strokes(element: &Graphic<'_>, style: stroke::StyleColumns, out: &mut Vec<stroke::StyledStroke>) {
	match element {
		Graphic::Stroke(stroke) if !stroke.is_empty() && stroke.is_valid() => out.push(stroke::StyledStroke {
			color: style.color,
			diameter: style.diameter,
			hardness: style.hardness,
			flow: style.flow,
			stroke: stroke.clone(),
		}),
		Graphic::GraphicList(nested) => {
			for child in nested.iter_element_values() {
				collect_styled_strokes(child, style, out);
			}
		}
		_ => {}
	}
}

#[node_macro::node(category(""), inject_scope)]
fn basic_brush_pipeline(
	_ctx: impl Ctx,
	#[scope(ProtoNodeIdentifier::new("graphene_std::platform_application_io::WgpuExecutorNode"))] executor: wgpu_executor::WgpuExecutorHandle,
	#[data] pipeline: WgpuPipelineCache,
) -> WgpuPipelineCache {
	executor.pipeline_init::<BasicBrushPipeline>(pipeline);
	pipeline.clone()
}
