use crate::raster_kernel::raster_kernel_pipeline;
use core_types::list::Item;
use core_types::{Color, Ctx, Interrupt, NodeError};
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Graphic, Vector, stamp_effect};
use vector_types::Gradient;
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::raster_effects::evaluate_raster_effects;

/// Content an effect can be carried out on. A GPU raster has the effect carried out into its pixels at once, with the same kernels the renderer uses,
/// while other content has it stamped onto its effects, to be carried out wherever it's rendered or read.
pub trait ApplyEffect: Sized {
	fn apply_effect(item: Item<Self>, operation: EffectOperation, _pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let mut item = item;
		stamp_effect(&mut item, Effect::new(&operation));
		Ok(item)
	}
}

impl ApplyEffect for Graphic {}
impl ApplyEffect for Vector {}
impl ApplyEffect for Raster<CPU> {}
impl ApplyEffect for Color {}
impl ApplyEffect for Gradient {}
impl ApplyEffect for String {}

impl ApplyEffect for Raster<GPU> {
	fn apply_effect(item: Item<Self>, operation: EffectOperation, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let mut item = item;
		stamp_effect(&mut item, Effect::new(&operation));
		evaluate_raster_effects(item, pipeline)
	}
}

fn apply_effect<T: ApplyEffect>(content: Item<T>, operation: EffectOperation, pipeline: Item<WgpuPipelineCache>) -> Result<Item<T>, Interrupt> {
	Ok(T::apply_effect(content, operation, pipeline.element())?)
}

/// Blurs the input graphics, whether vector shapes, images, or text.
#[node_macro::node(category("Raster: Filter"), icon("NodeBlur"))]
fn blur<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to blur.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String)]
	content: Item<T>,
	/// How far the blur spreads each point of the content.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	radius: Item<f64>,
	/// Use a box kernel instead of a circular Gaussian kernel, which averages evenly and leaves boxy artifacts.
	box_blur: Item<bool>,
	/// Opt to incorrectly blend the colors in gamma space for compatibility with the results from other software.
	gamma: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::Blur {
			radius: *radius.element(),
			box_blur: *box_blur.element(),
			gamma: *gamma.element(),
		},
		pipeline,
	)
}

/// Casts a soft shadow of the input graphics' silhouette beneath them.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn drop_shadow<T: ApplyEffect>(
	_: impl Ctx,
	/// The content casting the shadow.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How far the shadow sits from the content.
	#[unit(" px")]
	#[default(4., 4.)]
	offset: Item<DVec2>,
	/// How soft the shadow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(4.)]
	blur: Item<f64>,
	/// How far the shadow grows past the content's silhouette before it softens.
	#[unit(" px")]
	#[hard(0..)]
	spread: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::DropShadow {
			color: *color.element(),
			offset: *offset.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		},
		pipeline,
	)
}

/// Casts a soft shadow inward from the edges of the input graphics, as if they were cut out of the surface.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn inner_shadow<T: ApplyEffect>(
	_: impl Ctx,
	/// The content the shadow falls inside.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String)]
	content: Item<T>,
	/// The shadow's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How far the shadow sits from the edges it falls from.
	#[unit(" px")]
	#[default(4., 4.)]
	offset: Item<DVec2>,
	/// How soft the shadow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(4.)]
	blur: Item<f64>,
	/// How far the shadow reaches in from the content's edges before it softens.
	#[unit(" px")]
	#[hard(0..)]
	spread: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::InnerShadow {
			color: *color.element(),
			offset: *offset.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		},
		pipeline,
	)
}

/// Surrounds the input graphics with a soft glow of their silhouette.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn outer_glow<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to glow around.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String)]
	content: Item<T>,
	/// The glow's color, with its opacity.
	#[default("#ffffbebf")]
	color: Item<Color>,
	/// How soft the glow's edges are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(8.)]
	blur: Item<f64>,
	/// How far the glow grows past the content's silhouette before it softens.
	#[unit(" px")]
	#[hard(0..)]
	spread: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::OuterGlow {
			color: *color.element(),
			blur: *blur.element(),
			spread: *spread.element(),
		},
		pipeline,
	)
}
