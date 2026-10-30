use crate::raster_kernel::raster_kernel_pipeline;
use core_types::list::Item;
use core_types::ops::Convert;
use core_types::transform::Footprint;
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

/// Uploads an image to the GPU and carries out its effects into the pixels, at the image's own resolution, for what comes next to run on the GPU right away.
#[node_macro::node(category("Raster"), icon("NodeUploadTexture"))]
fn eager(
	_: impl Ctx,
	/// The image to upload, with any effects it carries.
	image: Item<Raster<CPU>>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	let pipeline = pipeline.element();
	let executor = pipeline.executor().cloned().ok_or_else(|| NodeError::new("A GPU is required to upload a raster"))?;

	let (image, attributes) = image.into_parts();
	let uploaded: Raster<GPU> = image.convert(Footprint::DEFAULT, executor);

	Ok(evaluate_raster_effects(Item::from_parts(uploaded, attributes), pipeline)?)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::ATTR_TRANSFORM;
	use core_types::list::ATTR_EFFECTS;
	use core_types::ops::ConvertAsync;
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
}
