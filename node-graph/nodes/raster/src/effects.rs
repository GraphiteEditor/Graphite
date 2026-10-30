use crate::raster_kernel::raster_kernel_pipeline;
use core_types::blending::BlendMode;
use core_types::list::{ATTR_EFFECTS, ATTR_GRADIENT_HUE_DIRECTION, ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, Item, List};
use core_types::{ATTR_OPACITY_FILL, CacheHash, Color, Ctx, Interrupt, NodeError};
use glam::DVec2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Adjustment, BevelDirection, BevelStyle, Effect, EffectOperation, Effects, Generator, GlowSource, Graphic, Vector, stamp_effect};
use raster_kernels::adjustments::{adjust_color, adjust_gradient, preserves_alpha};
use std::hash::{DefaultHasher, Hasher};
use std::sync::{Arc, Mutex, PoisonError};
use vector_types::vector::style::StrokeAlign;
use vector_types::{Gradient, GradientSettings};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::raster_effects::{evaluate_raster_effects, upload_raster};
use wgpu_executor::raster_kernel::adjust_raster;

/// Content an effect can be carried out on. A GPU raster has the effect carried out into its pixels at once, with the same kernels the renderer uses,
/// while other content has it stamped onto its effects, to be carried out wherever it's rendered or read.
pub trait ApplyEffect: Sized {
	fn apply_effect(item: Item<Self>, operation: EffectOperation, _pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let mut item = item;
		stamp_effect(&mut item, Effect::new(&operation));
		Ok(item)
	}

	/// The item with an adjustment applied after everything done to it so far.
	fn apply_adjustment(item: Item<Self>, adjustment: Adjustment, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		Self::apply_effect(item, EffectOperation::Adjustment(adjustment), pipeline)
	}
}

impl ApplyEffect for Graphic {}
impl ApplyEffect for Vector {}
impl ApplyEffect for Raster<CPU> {}
impl ApplyEffect for String {}
impl ApplyEffect for Generator {}

/// A color has no pixels for an effect to read, so an adjustment is carried out on the color itself with its kernel's mapping.
impl ApplyEffect for Color {
	fn apply_adjustment(item: Item<Self>, adjustment: Adjustment, _: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let mut item = item;
		*item.element_mut() = adjust_color(&adjustment, *item.element());
		Ok(item)
	}
}

/// A gradient has no pixels for an effect to read, so an adjustment is carried out along its ramp with its kernel's mapping.
impl ApplyEffect for Gradient {
	fn apply_adjustment(item: Item<Self>, adjustment: Adjustment, _: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let mut item = item;
		let (adjusted, settings) = adjust_gradient(&adjustment, item.element(), GradientSettings::from_item_attributes(&item));
		*item.element_mut() = adjusted;

		item.set_attribute(ATTR_GRADIENT_SPACE, settings.space);
		item.set_attribute(ATTR_GRADIENT_INTERPOLATION, settings.interpolation);
		item.set_attribute(ATTR_GRADIENT_HUE_DIRECTION, settings.hue_direction);
		Ok(item)
	}
}

impl ApplyEffect for Raster<GPU> {
	fn apply_effect(item: Item<Self>, operation: EffectOperation, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		let mut item = item;
		stamp_effect(&mut item, Effect::new(&operation));
		evaluate_raster_effects(item, pipeline)
	}

	fn apply_adjustment(item: Item<Self>, adjustment: Adjustment, pipeline: &WgpuPipelineCache) -> Result<Item<Self>, NodeError> {
		// With a fill to fade the content first, an adjustment changing alpha goes through the effects that apply it in that order
		let fill = item.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.);
		if item.attribute::<Effects>(ATTR_EFFECTS).is_some() || (fill < 1. && !preserves_alpha(&adjustment)) {
			return Self::apply_effect(item, EffectOperation::Adjustment(adjustment), pipeline);
		}

		let (raster, attributes) = item.into_parts();
		Ok(Item::from_parts(adjust_raster(&adjustment, raster, pipeline)?, attributes))
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
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
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
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
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
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
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
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
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

/// Glows inward from the edges of the input graphics, or outward from their center.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn inner_glow<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to glow within.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
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
	/// How far the glow grows into the content before it softens.
	#[unit(" px")]
	#[hard(0..)]
	choke: Item<f64>,
	/// Whether the glow starts from the edges or the center.
	source: Item<GlowSource>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::InnerGlow {
			color: *color.element(),
			blur: *blur.element(),
			choke: *choke.element(),
			source: *source.element(),
		},
		pipeline,
	)
}

/// Shades the inside of the input graphics with soft, glossy bands that follow their shape.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn satin<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to shade.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The shading's color, with its opacity.
	#[default("#00000080")]
	color: Item<Color>,
	/// How far apart the bands are drawn, and in which direction.
	#[unit(" px")]
	#[default(10., 4.)]
	offset: Item<DVec2>,
	/// How soft the bands are.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(14.)]
	blur: Item<f64>,
	/// Shade where the bands overlap rather than where they don't.
	#[default(true)]
	invert: Item<bool>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::Satin {
			color: *color.element(),
			offset: *offset.element(),
			blur: *blur.element(),
			invert: *invert.element(),
		},
		pipeline,
	)
}

/// Draws a band of color along the edge of the input graphics, outside, inside, or centered on it.
#[node_macro::node(category("Raster: Effect"), icon("NodeBlur"))]
fn outline<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to outline.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The outline's color, with its opacity.
	#[default(Color::BLACK)]
	color: Item<Color>,
	/// How thick the outline is.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(3.)]
	weight: Item<f64>,
	/// Whether the outline is drawn outside the edge, inside it, or centered on it.
	#[default(StrokeAlign::Outside)]
	align: Item<StrokeAlign>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::Outline {
			color: *color.element(),
			weight: *weight.element(),
			align: *align.element(),
		},
		pipeline,
	)
}

/// Raises the input graphics into a lit relief, with highlights on the slopes facing the light and shadows on those facing away.
#[node_macro::node(name("Bevel & Emboss"), category("Raster: Effect"), icon("NodeBlur"))]
fn bevel_and_emboss<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to raise.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// Where the slope lies: inside the edge, outside it, across it, or across it with the edge pressed in.
	style: Item<BevelStyle>,
	/// How steep the slope is.
	#[unit("%")]
	#[range]
	#[hard(0..)]
	#[soft(..1000)]
	#[default(100.)]
	depth: Item<f64>,
	/// Whether the slope rises out of the surface or sinks into it.
	direction: Item<BevelDirection>,
	/// How wide the slope is.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	#[default(5.)]
	size: Item<f64>,
	/// How much the shading is blurred.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..16)]
	soften: Item<f64>,
	/// The direction the light comes from.
	#[unit("°")]
	#[range]
	#[soft(-180..180)]
	#[default(120.)]
	angle: Item<f64>,
	/// How high above the surface the light is, from the horizon to straight overhead.
	#[unit("°")]
	#[range]
	#[hard(0..90)]
	#[default(30.)]
	altitude: Item<f64>,
	/// The highlight's color, with its opacity.
	#[default("#ffffffbf")]
	highlight_color: Item<Color>,
	/// The shadow's color, with its opacity.
	#[default("#000000bf")]
	shadow_color: Item<Color>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	apply_effect(
		content,
		EffectOperation::BevelAndEmboss {
			style: *style.element(),
			depth: *depth.element(),
			direction: *direction.element(),
			size: *size.element(),
			soften: *soften.element(),
			angle: *angle.element(),
			altitude: *altitude.element(),
			highlight_color: *highlight_color.element(),
			shadow_color: *shadow_color.element(),
		},
		pipeline,
	)
}

/// Paints over the input graphics within their shape, mixing the paint with their colors by a blend mode.
#[node_macro::node(category("Raster: Effect"), icon("NodeColorOverlay"))]
fn paint_overlay<T: ApplyEffect>(
	_: impl Ctx,
	/// The content to paint over.
	#[implementations(Raster<CPU>, Graphic, Vector, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The color, gradient, image, or other graphic to paint with. A gradient or image without its own placement stretches across the content.
	#[default(Color::BLACK)]
	paint: Item<Graphic>,
	/// How the paint mixes with the content's colors.
	blend_mode: Item<BlendMode>,
	/// How strongly the paint shows.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let (blend_mode, opacity) = (blend_mode.into_element(), opacity.into_element());

	// The paint is the element alone, since the wire envelope's attributes would nest it as a group
	Ok(match paint.into_element() {
		Graphic::None(_) | Graphic::NoneList(_) => content,
		// A solid paint mixes the same into every pixel, as an adjustment
		Graphic::Color(color) => {
			let color = *color.element();
			T::apply_adjustment(content, Adjustment::ColorOverlay { color, blend_mode, opacity }, pipeline.element())?
		}
		paint => T::apply_effect(content, EffectOperation::PaintOverlay { paint, blend_mode, opacity }, pipeline.element())?,
	})
}

/// Uploads an image to the GPU and carries out its effects into the pixels, at the image's own resolution, for what comes next to run on the GPU right away.
#[node_macro::node(category("Raster"), icon("NodeUploadTexture"))]
fn eager(
	_: impl Ctx,
	/// The image to upload, with any effects it carries.
	image: Item<Raster<CPU>>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	Ok(upload_raster(image, pipeline.element())?)
}

/// The `input_adapter` row uploading a CPU raster wire for an input that only takes GPU rasters, by the same steps as the Eager node.
/// The upload is kept until the wire carries other pixels or effects, so it's made once however often the consuming node runs.
#[node_macro::node(category("_Adapters"), skip_impl)]
fn upload_item(_: impl Ctx, value: Item<Raster<CPU>>, pipeline: Item<WgpuPipelineCache>, #[data] cache: Arc<Mutex<Option<(u64, Item<Raster<GPU>>)>>>) -> Result<Item<Raster<GPU>>, Interrupt> {
	Ok(uploaded(cache, &value, || upload_raster(value.clone(), pipeline.element()))?)
}

/// The `List` counterpart of `upload_item`.
#[node_macro::node(category("_Adapters"), skip_impl)]
fn upload_list(_: impl Ctx, value: List<Raster<CPU>>, pipeline: Item<WgpuPipelineCache>, #[data] cache: Arc<Mutex<Option<(u64, List<Raster<GPU>>)>>>) -> Result<List<Raster<GPU>>, Interrupt> {
	let upload = || value.clone().into_iter().map(|item| upload_raster(item, pipeline.element())).collect::<Result<List<_>, _>>();
	Ok(uploaded(cache, &value, upload)?)
}

/// The `input_adapter` row passing a GPU raster wire through, taking the pipeline only since every row under one identifier takes the same inputs.
#[node_macro::node(category("_Adapters"), skip_impl)]
fn pass_raster<'i, T: 'i + Send>(_: impl Ctx, value: T, _pipeline: Item<WgpuPipelineCache>) -> T {
	value
}

/// The result of `upload` for `source`, reusing the last one if it was for the same pixels and effects.
fn uploaded<S: CacheHash, T: Clone>(cache: &Mutex<Option<(u64, T)>>, source: &S, upload: impl FnOnce() -> Result<T, NodeError>) -> Result<T, NodeError> {
	let mut hasher = DefaultHasher::new();
	source.cache_hash(&mut hasher);
	let hash = hasher.finish();

	let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
	if let Some((_, result)) = cache.as_ref().filter(|(cached_hash, _)| *cached_hash == hash) {
		return Ok(result.clone());
	}

	let result = upload()?;
	*cache = Some((hash, result.clone()));
	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::ATTR_TRANSFORM;
	use core_types::Node;
	use core_types::context::OwnedContextImpl;
	use core_types::list::ATTR_EFFECTS;
	use core_types::ops::{Convert, ConvertAsync};
	use core_types::poll::Poll;
	use core_types::transform::Footprint;
	use core_types::value::PrecomputedItemNode;
	use glam::{DAffine2, UVec2};
	use graphic_types::Effects;
	use graphic_types::raster_types::{Image, Storage};
	use wgpu_executor::WgpuExecutor;
	use wgpu_executor::shader_runtime::raster_kernel_runtime::RasterKernelRuntime;

	const SHADOW: EffectOperation = EffectOperation::DropShadow {
		color: Color::BLACK,
		offset: DVec2::new(2., 0.),
		blur: 0.,
		spread: 0.,
	};

	#[test]
	fn a_cpu_raster_is_stamped_and_a_gpu_raster_carried_out() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			let cpu = Raster::new_cpu(Image::new(4, 4, Color::WHITE));
			let gpu: Raster<GPU> = cpu.clone().convert(Footprint::DEFAULT, executor.clone());
			let transform = DAffine2::from_scale(DVec2::splat(4.));

			// The CPU raster keeps its pixels and carries the effect
			let stamped = Raster::<CPU>::apply_effect(Item::new_from_element(cpu).with_attribute(ATTR_TRANSFORM, transform), SHADOW, &pipeline).unwrap();
			assert_eq!(stamped.attribute::<Effects>(ATTR_EFFECTS).map(Effects::len), Some(1));
			assert_eq!((stamped.element().width, stamped.element().height), (4, 4));

			// The GPU raster grows by the two pixels the shadow reaches to each side and grows its coverage to hold them, keeping its transform
			let carried_out = Raster::<GPU>::apply_effect(Item::new_from_element(gpu).with_attribute(ATTR_TRANSFORM, transform), SHADOW, &pipeline).unwrap();
			assert!(carried_out.attribute::<Effects>(ATTR_EFFECTS).is_none());
			assert_eq!(carried_out.element().size(), UVec2::splat(8));
			let item_transform: DAffine2 = carried_out.attribute_cloned_or_default(ATTR_TRANSFORM);
			assert_eq!(item_transform, transform);
			let placement = item_transform * carried_out.element().coverage();
			assert_eq!(placement.transform_point2(DVec2::ZERO), DVec2::splat(-2.));
			assert_eq!(placement.transform_point2(DVec2::ONE), DVec2::splat(6.));
		});
	}

	/// The color of the raster's pixel at a point in the document, through its transform and coverage, or transparent outside its pixels.
	async fn color_at(item: &Item<Raster<GPU>>, point: DVec2, executor: &WgpuExecutor) -> Color {
		let transform: DAffine2 = item.attribute_cloned_or_default(ATTR_TRANSFORM);
		let placement = transform * item.element().coverage();
		let raster: Raster<CPU> = item.element().clone().convert(Footprint::DEFAULT, executor.clone()).await;

		let pixel = (DAffine2::from_scale(DVec2::new(raster.width as f64, raster.height as f64)) * placement.inverse())
			.transform_point2(point)
			.floor();
		if pixel.x < 0. || pixel.y < 0. || pixel.x >= raster.width as f64 || pixel.y >= raster.height as f64 {
			return Color::TRANSPARENT;
		}
		raster.data[(pixel.y as u32 * raster.width + pixel.x as u32) as usize]
	}

	fn assert_color(actual: Color, expected: Color) {
		let channels = |color: Color| [color.r(), color.g(), color.b(), color.a()];
		let close = channels(actual).into_iter().zip(channels(expected)).all(|(actual, expected)| (actual - expected).abs() < 0.01);
		assert!(close, "{actual:?} should be {expected:?}");
	}

	#[test]
	fn carrying_out_effects_keeps_the_transform_and_the_pixels_in_place() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			let gpu: Raster<GPU> = Raster::new_cpu(Image::new(4, 4, Color::WHITE)).convert(Footprint::DEFAULT, executor.clone());
			let transform = DAffine2::from_translation(DVec2::new(10., 20.)) * DAffine2::from_scale(DVec2::splat(4.));

			let carried_out = Raster::<GPU>::apply_effect(Item::new_from_element(gpu).with_attribute(ATTR_TRANSFORM, transform), SHADOW, &pipeline).unwrap();

			assert_eq!(carried_out.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM), transform);
			assert_ne!(carried_out.element().coverage(), DAffine2::IDENTITY);
			// The content stays over [10, 14] and its shadow reaches two document pixels to the right of it
			assert_color(color_at(&carried_out, DVec2::new(12.5, 22.5), &executor).await, Color::WHITE);
			assert_color(color_at(&carried_out, DVec2::new(15.5, 22.5), &executor).await, Color::BLACK);
			assert_color(color_at(&carried_out, DVec2::new(9.5, 22.5), &executor).await, Color::TRANSPARENT);
		});
	}

	#[test]
	fn effects_measure_and_place_through_a_rasters_coverage() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			// The pixels cover the middle of the item's unit square, the document area [2, 6], at one pixel per document pixel where the transform alone would suggest two per pixel
			let coverage = DAffine2::from_translation(DVec2::splat(0.25)) * DAffine2::from_scale(DVec2::splat(0.5));
			let transform = DAffine2::from_scale(DVec2::splat(8.));
			let gpu: Raster<GPU> = Raster::new_cpu(Image::new(4, 4, Color::WHITE)).convert(Footprint::DEFAULT, executor.clone());
			let item = Item::new_from_element(gpu.with_coverage(coverage)).with_attribute(ATTR_TRANSFORM, transform);

			let carried_out = Raster::<GPU>::apply_effect(item, SHADOW, &pipeline).unwrap();

			assert_eq!(carried_out.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM), transform);
			assert_eq!(carried_out.element().size(), UVec2::splat(8));
			assert_color(color_at(&carried_out, DVec2::new(3.5, 4.5), &executor).await, Color::WHITE);
			// The two document pixel offset lands the shadow over [4, 8], which it would only half reach if measured through the transform alone
			assert_color(color_at(&carried_out, DVec2::new(7.5, 4.5), &executor).await, Color::BLACK);
			assert_color(color_at(&carried_out, DVec2::new(1.5, 4.5), &executor).await, Color::TRANSPARENT);
		});
	}

	#[test]
	fn adjusting_a_raster_follows_its_earlier_effects_and_stays_stamped_until_uploaded() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			// A white square in the middle of a transparent image, placed one pixel per document unit, then blurred
			let mut image = Image::new(20, 20, Color::TRANSPARENT);
			for y in 5..15 {
				for x in 5..15 {
					image.data[y * 20 + x] = Color::WHITE;
				}
			}
			let transform = DAffine2::from_scale(DVec2::splat(20.));
			let item = Item::new_from_element(Raster::new_cpu(image)).with_attribute(ATTR_TRANSFORM, transform);
			let blurred = Raster::<CPU>::apply_effect(
				item,
				EffectOperation::Blur {
					radius: 1.,
					box_blur: false,
					gamma: false,
				},
				&pipeline,
			)
			.unwrap();

			// The pixels wait for the stamps, with the adjustment after the blur
			let adjusted = Raster::<CPU>::apply_adjustment(blurred, Adjustment::MakeOpaque, &pipeline).unwrap();
			assert_eq!(adjusted.attribute::<Effects>(ATTR_EFFECTS).map(Effects::len), Some(2));
			assert_eq!(adjusted.element().data[0], Color::TRANSPARENT);

			// Just outside the square, the blur's faint white becomes opaque white, where making it opaque first would have blurred in black
			let uploaded = super::eager((), adjusted, Item::new_from_element(pipeline.clone())).unwrap();
			assert_color(color_at(&uploaded, DVec2::new(4.5, 10.5), &executor).await, Color::WHITE);
		});
	}

	#[test]
	fn a_gpu_raster_is_adjusted_at_once_in_place_of_a_stamp() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			let coverage = DAffine2::from_translation(DVec2::splat(0.25)) * DAffine2::from_scale(DVec2::splat(0.5));
			let gpu: Raster<GPU> = Raster::new_cpu(Image::new(2, 2, Color::WHITE)).convert(Footprint::DEFAULT, executor.clone());
			let item = Item::new_from_element(gpu.with_coverage(coverage)).with_attribute(ATTR_OPACITY_FILL, 0.5_f64);

			let adjusted = Raster::<GPU>::apply_adjustment(item, Adjustment::Invert, &pipeline).unwrap();
			assert!(adjusted.attribute::<Effects>(ATTR_EFFECTS).is_none());
			assert_eq!(adjusted.element().coverage(), coverage, "the pixels stay where they were");
			assert_eq!(adjusted.attribute_cloned_or::<f64>(ATTR_OPACITY_FILL, 1.), 0.5, "the fill stays for the render");
			let raster: Raster<CPU> = adjusted.into_element().convert(Footprint::DEFAULT, executor.clone()).await;
			assert_color(raster.data[0], Color::BLACK);
		});
	}

	#[test]
	fn paint_overlay_multiplies_a_color_into_the_content() {
		let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);

		let result = super::paint_overlay(
			(),
			Item::new_from_element(image_color),
			Item::new_from_element(Graphic::from(Color::GREEN)),
			BlendMode::Multiply.into(),
			100_f64.into(),
			Item::default(),
		);

		// Multiplying by green keeps only the green channel, and the content keeps its own alpha
		assert_eq!(result.unwrap().into_element(), Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
	}

	#[test]
	fn the_input_adapter_uploads_a_raster_once_until_its_pixels_or_effects_change() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);
			let pipeline = Item::new_from_element(pipeline);

			let cache = Default::default();
			let upload = |item: &Item<Raster<CPU>>| super::upload_item((), &cache, item.clone(), pipeline.clone()).unwrap();

			let plain = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::WHITE)));
			let first = upload(&plain);
			assert_eq!(upload(&plain).element().texture(), first.element().texture(), "the same pixels stay uploaded");

			// Stamps carried out before anything reads the pixels, leaving none behind
			let inverted = Raster::<CPU>::apply_adjustment(plain, Adjustment::Invert, pipeline.element()).unwrap();
			let second = upload(&inverted);
			assert_ne!(second.element().texture(), first.element().texture(), "other effects are uploaded anew");
			assert!(second.attribute::<Effects>(ATTR_EFFECTS).is_none());
			let raster: Raster<CPU> = second.into_element().convert(Footprint::DEFAULT, executor.clone()).await;
			assert_color(raster.data[0], Color::BLACK);
		});
	}

	#[test]
	fn the_input_adapter_keeps_its_upload_when_the_footprint_changes() {
		futures::executor::block_on(async {
			let Some(executor) = WgpuExecutor::new().await else { return };
			let pipeline = WgpuPipelineCache::default();
			executor.pipeline_init::<RasterKernelRuntime>(&pipeline);

			let raster = Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::WHITE)));
			let adapter = UploadItemNode::new(PrecomputedItemNode::new(raster), PrecomputedItemNode::new(Item::new_from_element(pipeline)));

			let upload_under = |translation: DVec2| {
				let footprint = Footprint {
					transform: DAffine2::from_translation(translation),
					..Footprint::DEFAULT
				};
				let context = OwnedContextImpl::default().with_footprint(footprint).into_context();
				let Poll::Final(uploaded) = adapter.eval(&context) else {
					panic!("the upload should finish at once")
				};
				uploaded
			};

			let first = upload_under(DVec2::ZERO);
			let panned = upload_under(DVec2::new(500., -300.));
			assert_eq!(panned.element().texture(), first.element().texture(), "panning shouldn't upload the raster again");
		});
	}
}
