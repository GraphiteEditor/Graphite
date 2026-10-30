use crate::adjustments::{CellularDistanceFunction, CellularReturnType, DomainWarpType, FractalType, NoiseType};
use crate::gpu_params;
use crate::raster_kernel::{KernelFrame, RasterKernel, Wgsl, raster_kernel_pipeline, raster_placement, run_raster_kernel, union_frame};
use core_types::ATTR_TRANSFORM;
use core_types::context::{Ctx, ExtractFootprint};
use core_types::list::Item;
use core_types::math::bbox::Bbox;
use core_types::ops::Convert;
use core_types::transform::{Footprint, Transform};
use core_types::{Interrupt, NodeError};
use glam::{DAffine2, DVec2, UVec2};
use graphene_resource::Resource;
use graphic_types::generator::{GeneratorPattern, NoiseSettings};
use raster_types::Image;
use raster_types::{GPU, Raster, Storage};
use wgpu_executor::WgpuPipelineCache;
use wgpu_executor::raster_kernel::render_generator;

#[node_macro::node(category("Raster"), icon("NodeChannelsToImage"), name("Channels to Image"), memoize)]
fn channels_to_image(
	_: impl Ctx,
	_primary: (),
	#[expose] red: Item<Raster<GPU>>,
	#[expose] green: Item<Raster<GPU>>,
	#[expose] blue: Item<Raster<GPU>>,
	#[expose] alpha: Item<Raster<GPU>>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	// Each channel is placed by its transform into their `union_frame`, which takes the red channel's attributes with the frame as its coverage, and gives its luminance weighted by its alpha.
	// An empty channel gives zero, or full opacity for the alpha channel.
	let channels = [red, green, blue, alpha];
	if channels.iter().all(|channel| channel.element().is_empty()) {
		return Ok(Item::default());
	}

	let frame = union_frame(&channels.each_ref().map(raster_placement)).ok_or_else(|| NodeError::new("Every channel must be placed by an invertible transform"))?;
	let raster = run_raster_kernel(&CombineChannels {}, frame.kernel_inputs(&channels.each_ref().map(Item::element)), frame.size, pipeline.element())?;

	let [red, ..] = channels;
	let red_transform: DAffine2 = red.attribute_cloned_or_default(ATTR_TRANSFORM);
	let (_, attributes) = red.into_parts();
	Ok(Item::from_parts(raster.with_coverage(red_transform.inverse() * frame.transform), attributes))
}

#[node_macro::node(category("Raster"), icon("NodeMask"), memoize)]
fn mask(
	_: impl Ctx,
	/// The image to be masked.
	image: Item<Raster<GPU>>,
	/// The stencil to be used for masking.
	#[expose]
	stencil: Item<Raster<GPU>>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	// The stencil is sampled bilinearly where each image pixel's center lies, extending its edge pixels beyond it
	if image.element().is_empty() || stencil.element().is_empty() {
		return Ok(image);
	}

	let (image_size, image_transform) = raster_placement(&image);
	let (stencil_size, stencil_transform) = raster_placement(&stencil);
	if image_transform.matrix2.determinant() == 0. || stencil_transform.matrix2.determinant() == 0. {
		return Ok(image);
	}

	let frame = KernelFrame::new(image_size, image_transform, &[(image_size, image_transform), (stencil_size, stencil_transform)]);
	let to_stencil = frame.output_to_texel[1];
	let kernel = MaskStencil {
		stencil_axes: to_stencil.matrix2.to_cols_array().map(|value| value as f32),
		stencil_translation: to_stencil.translation.to_array().map(|value| value as f32),
	};
	let raster = run_raster_kernel(&kernel, frame.kernel_inputs(&[image.element(), stencil.element()]), frame.size, pipeline.element())?;

	let coverage = image.element().coverage();
	let (_, attributes) = image.into_parts();
	Ok(Item::from_parts(raster.with_coverage(coverage), attributes))
}

#[node_macro::node(category(""))]
fn extend_image_to_bounds(
	_: impl Ctx,
	image: Item<Raster<GPU>>,
	bounds: Item<DAffine2>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	let bounds = *bounds.element();
	if image.element().is_empty() {
		return Ok(image);
	}

	let (image_size, image_transform) = raster_placement(&image);
	let image_aabb = Bbox::unit().affine_transform(image_transform).to_axis_aligned_bbox();
	let bounds_aabb = Bbox::unit().affine_transform(bounds.transform()).to_axis_aligned_bbox();
	if image_transform.matrix2.determinant() == 0. || (image_aabb.contains(bounds_aabb.start) && image_aabb.contains(bounds_aabb.end)) {
		return Ok(image);
	}

	let orig_image_scale = image_size.as_dvec2();
	let layer_to_image_space = DAffine2::from_scale(orig_image_scale) * image_transform.inverse();
	let bounds_in_image_space = Bbox::unit().affine_transform(layer_to_image_space * bounds).to_axis_aligned_bbox();
	let new_start = bounds_in_image_space.start.floor().min(DVec2::ZERO);
	let new_scale = bounds_in_image_space.end.ceil().max(orig_image_scale) - new_start;
	let transform = image_transform * DAffine2::from_scale(1. / orig_image_scale) * DAffine2::from_translation(new_start) * DAffine2::from_scale(new_scale);

	let frame = KernelFrame::new(new_scale.as_uvec2(), transform, &[(image_size, image_transform)]);
	let offset = new_start.as_ivec2();
	let kernel = ExtendToBounds {
		offset_x: offset.x,
		offset_y: offset.y,
	};
	let raster = run_raster_kernel(&kernel, frame.kernel_inputs(&[image.element()]), frame.size, pipeline.element())?;

	let image_logical_transform: DAffine2 = image.attribute_cloned_or_default(ATTR_TRANSFORM);
	let (_, attributes) = image.into_parts();
	Ok(Item::from_parts(raster.with_coverage(image_logical_transform.inverse() * transform), attributes))
}

gpu_params! {
	#[derive(Clone)]
	struct CombineChannels {}
}

impl RasterKernel for CombineChannels {
	const INPUTS: usize = 4;
	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let red = input_0_at(coord);
			let green = input_1_at(coord);
			let blue = input_2_at(coord);
			let alpha = select(input_3_at(coord), vec4<f32>(1.), any(input_3_size() == vec2<i32>(0)));
			return vec4<f32>(luminance_rec_709(red) * red.a, luminance_rec_709(green) * green.a, luminance_rec_709(blue) * blue.a, luminance_rec_709(alpha) * alpha.a);
			"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct MaskStencil {
		stencil_axes: [f32; 4],
		stencil_translation: [f32; 2],
	}
}

impl RasterKernel for MaskStencil {
	const INPUTS: usize = 2;
	fn wgsl() -> Wgsl {
		Wgsl::function(
			r#"
fn stencil_luminance(texel: vec2<i32>) -> f32 {
	return luminance_rec_709(input_1_load(texel));
}

fn kernel(coord: vec2<i32>) -> vec4<f32> {
	let parameters = params();
	let axes = parameters.stencil_axes;
	let translation = parameters.stencil_translation;
	let center = vec2<f32>(coord) + 0.5;
	let texel = vec2<f32>(axes[0], axes[1]) * center.x + vec2<f32>(axes[2], axes[3]) * center.y + vec2<f32>(translation[0], translation[1]) - 0.5;
	let base = floor(texel);
	let t = texel - base;
	let corner = vec2<i32>(base);
	let top = mix(stencil_luminance(corner), stencil_luminance(corner + vec2<i32>(1, 0)), t.x);
	let bottom = mix(stencil_luminance(corner + vec2<i32>(0, 1)), stencil_luminance(corner + vec2<i32>(1, 1)), t.x);
	let color = input_0_load(coord);
	return vec4<f32>(color.rgb, color.a * mix(top, bottom, t.y));
}
"#,
		)
	}
}

gpu_params! {
	#[derive(Clone)]
	struct ExtendToBounds {
		offset_x: i32,
		offset_y: i32,
	}
}

impl RasterKernel for ExtendToBounds {
	const INPUTS: usize = 1;
	fn wgsl() -> Wgsl {
		Wgsl::body(
			r#"
			let texel = coord + vec2<i32>(offset_x, offset_y);
			if any(texel < vec2<i32>(0)) || any(texel >= input_0_size()) {
				return vec4<f32>(0.);
			}
			return input_0_load(texel);
			"#,
		)
	}
}

/// Displays an image from a file.
///
/// Reads PNG, JPG, GIF, WEBP, TIFF, BMP, TGA, ICO, HDR, and EXR files. Light brighter than white, as HDR and EXR files can hold, is clipped to white.
#[node_macro::node(category("Raster"), icon("NodeImage"), memoize)]
fn image(
	_: impl Ctx,
	_primary: (),
	/// The image file to display.
	#[widget(ParsedWidgetOverride::Custom = "image_file")]
	resource: Item<Resource>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	let resource = resource.into_element();
	if resource.is_empty() {
		return Ok(Item::default());
	}
	let image = Image::from_encoded(resource.as_ref()).map_err(NodeError::new)?;

	let executor = pipeline.element().executor().cloned().ok_or_else(|| NodeError::new("A GPU is required to load an image"))?;
	Ok(Item::new_from_element(Raster::new_cpu(image).convert(Footprint::DEFAULT, executor)))
}

/// Generates customizable procedural noise patterns.
#[node_macro::node(category("Raster"), icon("NodeNoisePattern"))]
#[allow(clippy::too_many_arguments)]
fn noise_pattern(
	ctx: impl ExtractFootprint + Ctx,
	_primary: (),
	#[default(true)] clip: Item<bool>,
	#[hard(0..)] seed: Item<i64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_scale")]
	#[default(10.)]
	scale: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_noise_type")] noise_type: Item<NoiseType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_domain_warp_type")] domain_warp_type: Item<DomainWarpType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_domain_warp_amplitude")]
	#[default(100.)]
	domain_warp_amplitude: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_type")] fractal_type: Item<FractalType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_octaves")]
	#[default(3)]
	#[hard(1..10)]
	fractal_octaves: Item<i64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_lacunarity")]
	#[default(2.)]
	fractal_lacunarity: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_gain")]
	#[default(0.5)]
	fractal_gain: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_fractal_weighted_strength")] fractal_weighted_strength: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_ping_pong_strength")]
	#[default(2.)]
	fractal_ping_pong_strength: Item<f64>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_cellular_distance_function")] cellular_distance_function: Item<CellularDistanceFunction>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_cellular_return_type")] cellular_return_type: Item<CellularReturnType>,
	#[widget(ParsedWidgetOverride::Custom = "noise_inspector_cellular_jitter")]
	#[default(1.)]
	cellular_jitter: Item<f64>,
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<Raster<GPU>>, Interrupt> {
	let (clip, seed, scale, domain_warp_amplitude) = (*clip.element(), *seed.element(), *scale.element(), *domain_warp_amplitude.element());
	let (fractal_octaves, fractal_lacunarity, fractal_gain) = (*fractal_octaves.element(), *fractal_lacunarity.element(), *fractal_gain.element());
	let (fractal_weighted_strength, fractal_ping_pong_strength, cellular_jitter) = (*fractal_weighted_strength.element(), *fractal_ping_pong_strength.element(), *cellular_jitter.element());
	let (noise_type, domain_warp_type, fractal_type) = (noise_type.into_element(), domain_warp_type.into_element(), fractal_type.into_element());
	let (cellular_distance_function, cellular_return_type) = (cellular_distance_function.into_element(), cellular_return_type.into_element());

	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	// TODO: Remove "clip" entirely (and its arbitrary 100x100 clipping square) once we have proper resolution-aware layer clipping
	const CLIPPING_SQUARE_SIZE: f64 = 100.;
	let logical_transform = if clip { DAffine2::from_scale(DVec2::splat(CLIPPING_SQUARE_SIZE)) } else { DAffine2::IDENTITY };

	let mut size = viewport_bounds.size();
	let mut offset = viewport_bounds.start;
	if clip {
		let image_bounds = Bbox::from_transform(logical_transform).to_axis_aligned_bbox();
		let intersection = viewport_bounds.intersect(&image_bounds);

		offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);
		size = intersection.size();
	}

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return Ok(Item::default().with_attribute(ATTR_TRANSFORM, logical_transform));
	}

	let transform = DAffine2::from_translation(offset) * DAffine2::from_scale(size);

	let footprint_scale = footprint.scale();
	let width = (size.x * footprint_scale.x) as u32;
	let height = (size.y * footprint_scale.y) as u32;
	if width == 0 || height == 0 {
		return Ok(Item::default().with_attribute(ATTR_TRANSFORM, logical_transform));
	}

	let noise = GeneratorPattern::Noise(NoiseSettings {
		seed,
		scale,
		noise_type,
		domain_warp_type,
		domain_warp_amplitude,
		fractal_type,
		fractal_octaves,
		fractal_lacunarity,
		fractal_gain,
		fractal_weighted_strength,
		fractal_ping_pong_strength,
		cellular_distance_function,
		cellular_return_type,
		cellular_jitter,
	});
	let raster = render_generator(&noise, [offset, offset + size], UVec2::new(width, height), pipeline.element())?;

	Ok(Item::new_from_element(raster.with_coverage(logical_transform.inverse() * transform)).with_attribute(ATTR_TRANSFORM, logical_transform))
}

#[node_macro::node(category("Raster"), icon("NodeMandelbrot"))]
fn mandelbrot(ctx: impl ExtractFootprint + Send, #[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>) -> Result<Item<Raster<GPU>>, Interrupt> {
	let footprint = ctx.footprint();
	let viewport_bounds = footprint.viewport_bounds_in_local_space();

	let image_bounds = Bbox::from_transform(DAffine2::IDENTITY).to_axis_aligned_bbox();
	let intersection = viewport_bounds.intersect(&image_bounds);
	let size = intersection.size();

	let offset = (intersection.start - image_bounds.start).max(DVec2::ZERO);

	// If the image would not be visible, return an empty image
	if size.x <= 0. || size.y <= 0. {
		return Ok(Item::default());
	}

	let scale = footprint.scale();
	let width = (size.x * scale.x) as u32;
	let height = (size.y * scale.y) as u32;
	if width == 0 || height == 0 {
		return Ok(Item::default());
	}

	let raster = render_generator(&GeneratorPattern::Mandelbrot, [offset, offset + size], UVec2::new(width, height), pipeline.element())?;

	Ok(Item::new_from_element(raster.with_coverage(DAffine2::from_translation(offset) * DAffine2::from_scale(size))))
}
