//! Renders scenes that stress Vello GPU on the machine's GPU, skipping when none is available.
//!
//! wgpu's environment variables pick the backend and shader compiler they run on, so one machine can cover several. On Windows, for instance,
//! `WGPU_BACKEND` takes `dx12`, `vulkan`, or `gl`, `WGPU_DX12_COMPILER` takes `fxc` or `dxc`, and `WGPU_DEBUG=0` builds shaders as a release build does.

use crate::WgpuExecutor;
use glam::{DAffine2, UVec2};
use rendering::vello_scene_builder::VelloSceneBuilder;
use vello_gpu::kurbo::{Affine, BezPath, Line, ParamCurve, PathSeg, Point, Rect, Stroke, Vec2};
use vello_gpu::peniko::{self, Color, Fill};

/// The executor every test shares, or `None` without a GPU.
fn executor() -> Option<&'static WgpuExecutor> {
	static EXECUTOR: std::sync::OnceLock<Option<WgpuExecutor>> = std::sync::OnceLock::new();
	EXECUTOR
		.get_or_init(|| futures::executor::block_on(crate::WgpuContext::for_tests()).and_then(WgpuExecutor::with_context))
		.as_ref()
}

/// Held while a test uses the shared GPU, since another test's work can delay a render's readback.
fn take_turn() -> std::sync::MutexGuard<'static, ()> {
	static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
	TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Renders the scene `draw` draws into straight-alpha RGBA8 pixels, or `None` without a GPU.
fn render(size: UVec2, draw: impl FnMut(&mut VelloSceneBuilder)) -> Option<Vec<[u8; 4]>> {
	render_timed(size, draw).map(|(pixels, _)| pixels)
}

/// Renders the scene `draw` draws into straight-alpha RGBA8 pixels, with how long the render and its readback took, or `None` without a GPU.
fn render_timed(size: UVec2, mut draw: impl FnMut(&mut VelloSceneBuilder)) -> Option<(Vec<[u8; 4]>, std::time::Duration)> {
	let executor = executor()?;
	let _turn = take_turn();
	let start = std::time::Instant::now();

	let texture = match executor.render_vello(size, DAffine2::IDENTITY, &mut draw) {
		Ok(texture) => texture,
		Err(error) => panic!("Render failed: {error}"),
	};
	let pixels = executor.read_texture_pixels(&texture).expect("Failed to read back the render");

	Some((pixels.as_chunks::<4>().0.to_vec(), start.elapsed()))
}

/// A circle drawn with four cubic segments, as Graphite's vector shapes are, rather than kurbo's tolerance-dependent count.
fn circle(center: (f64, f64), radius: f64) -> BezPath {
	const HANDLE: f64 = 0.552_284_749_830_793_4;
	let (x, y) = center;
	let (r, h) = (radius, radius * HANDLE);

	let mut path = BezPath::new();
	path.move_to((x + r, y));
	path.curve_to((x + r, y + h), (x + h, y + r), (x, y + r));
	path.curve_to((x - h, y + r), (x - r, y + h), (x - r, y));
	path.curve_to((x - r, y - h), (x - h, y - r), (x, y - r));
	path.curve_to((x + h, y - r), (x + r, y - h), (x + r, y));
	path.close_path();
	path
}

fn pixel(pixels: &[[u8; 4]], size: UVec2, x: u32, y: u32) -> [u8; 4] {
	pixels[(y * size.x + x) as usize]
}

const RED: Color = Color::from_rgb8(255, 0, 0);
const BLUE: Color = Color::from_rgb8(0, 0, 255);

#[test]
fn dense_strokes_render() {
	let size = UVec2::new(2048, 2048);
	let draw = |scene: &mut VelloSceneBuilder| {
		for i in 0..40_000 {
			let y = (i as f64 * 0.0511) % 2048.;
			scene.stroke(&Stroke::new(1.), Affine::IDENTITY, RED, None, &Line::new((0., y), (2048., y + 3.)));
		}
	};

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 1024, 1024)[3], 255);
}

#[test]
fn huge_circle_edge_lands_on_the_right_pixel() {
	let size = UVec2::new(256, 256);
	let draw = |scene: &mut VelloSceneBuilder| {
		for radius in [1e6, 1e9, 1e12] {
			scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((128. - radius, 128.), radius));
		}
	};

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 120, 128)[3], 255, "inside");
	assert_eq!(pixel(&pixels, size, 136, 128)[3], 0, "outside");
}

#[test]
fn huge_curves_through_the_viewport_render_quickly() {
	let size = UVec2::new(256, 256);
	for radius in [1e13, 1e14, 1e15] {
		let draw = |scene: &mut VelloSceneBuilder| {
			scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((128. - radius, 128.), radius));
			scene.stroke(&Stroke::new(4.), Affine::IDENTITY, RED, None, &circle((200. + radius, 128.), radius));
		};

		let Some((pixels, duration)) = render_timed(size, draw) else { return };
		println!("Radius {radius}: {duration:?}");
		assert!(duration.as_secs_f64() < 2., "radius {radius} took {duration:?}");
		assert_eq!(pixel(&pixels, size, 120, 128)[3], 255, "inside at radius {radius}");
		assert_eq!(pixel(&pixels, size, 160, 128)[3], 0, "outside at radius {radius}");
		assert_eq!(pixel(&pixels, size, 200, 128)[3], 255, "stroke at radius {radius}");
	}
}

#[test]
fn astronomical_fills_render_quickly() {
	let size = UVec2::new(256, 256);
	for radius in [1e18, 1e30, 1e100, 1e300] {
		let draw = |scene: &mut VelloSceneBuilder| scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((0., 0.), radius));

		let Some((pixels, duration)) = render_timed(size, draw) else { return };
		println!("Fill radius {radius}: {duration:?}");
		assert!(duration.as_secs_f64() < 2., "radius {radius} took {duration:?}");
		assert_eq!(pixel(&pixels, size, 128, 128)[3], 255, "inside at radius {radius}");
	}
}

#[test]
fn astronomical_strokes_render_quickly() {
	let size = UVec2::new(256, 256);
	for radius in [1e18, 1e30, 1e100, 1e300] {
		let draw = |scene: &mut VelloSceneBuilder| scene.stroke(&Stroke::new(1e3), Affine::IDENTITY, RED, None, &circle((0., 0.), radius));

		let Some((pixels, duration)) = render_timed(size, draw) else { return };
		println!("Stroke radius {radius}: {duration:?}");
		assert!(duration.as_secs_f64() < 2., "radius {radius} took {duration:?}");
		assert_eq!(pixel(&pixels, size, 128, 128)[3], 0, "far inside at radius {radius}");
	}
}

#[test]
fn enormous_line_renders() {
	let size = UVec2::new(256, 256);
	let draw = |scene: &mut VelloSceneBuilder| scene.stroke(&Stroke::new(4.), Affine::IDENTITY, RED, None, &Line::new((-1e30, 100.), (1e30, 100.)));

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 128, 100)[3], 255);
}

#[test]
fn infinite_line_renders() {
	let size = UVec2::new(256, 256);
	let mut path = BezPath::new();
	path.move_to((-5., f64::INFINITY));
	path.line_to((5., f64::NEG_INFINITY));
	let draw = |scene: &mut VelloSceneBuilder| scene.stroke(&Stroke::new(4.), Affine::translate((128., 0.)), RED, None, &path);

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 128, 128)[3], 255);
}

/// A view at `zoom` and `tilt` that places the document point `target` at the pixel `pixel`.
fn deep_view(zoom: f64, tilt: f64, target: Point, pixel: Point) -> Affine {
	Affine::translate(pixel.to_vec2()) * Affine::rotate(tilt) * Affine::scale(zoom) * Affine::translate(-target.to_vec2())
}

#[test]
fn deep_zoomed_edges_land_on_the_right_pixel() {
	let size = UVec2::new(256, 256);
	let middle = Point::new(128., 128.);
	let (center, radius) = ((1234.5678, -987.6543), 100.);
	let path = circle(center, radius);

	// The middle of a segment, where a stroke's fitted outline strays furthest, and outward from the circle by the segment's symmetry
	let Some(PathSeg::Cubic(segment)) = path.get_seg(2) else {
		panic!("The circle's second segment is a cubic")
	};
	let on_curve = segment.eval(0.5);
	let outward = (on_curve - Point::from(center)).normalize();

	for zoom in [1e4, 1e7, 1e10] {
		for tilt in [0., 0.3, std::f64::consts::FRAC_PI_2, 2.4] {
			// The edge belongs at the middle pixel, with the shape covering what's inward of it
			for (label, target, stroke) in [("fill", on_curve, None), ("stroke", on_curve + outward, Some(Stroke::new(2.)))] {
				let view = deep_view(zoom, tilt, target, middle);
				let Some(pixels) = render(size, |scene| match &stroke {
					Some(stroke) => scene.stroke(stroke, view, RED, None, &path),
					None => scene.fill(Fill::NonZero, view, RED, None, &path),
				}) else {
					return;
				};

				// Walking inward from outside to the first mostly covered pixel
				let screen_outward = (view * (target + outward / zoom)) - middle;
				let covered_at = (-40..=40_i32).rev().find(|&distance| {
					let point = middle + screen_outward * distance as f64;
					pixel(&pixels, size, point.x as u32, point.y as u32)[3] >= 128
				});
				assert!(
					covered_at.is_some_and(|distance| distance.abs() <= 2),
					"the {label} edge at zoom {zoom} and tilt {tilt} lands {covered_at:?} pixels outward"
				);
			}
		}
	}
}

#[test]
fn deep_zoomed_images_blend_their_texels_precisely() {
	let size = UVec2::new(256, 256);

	// A red texel beside a blue one, sampled as Graphite draws images, viewed a quarter texel past the edge between them
	let image = peniko::ImageBrush::new(peniko::ImageData {
		data: [255, 0, 0, 255, 0, 0, 255, 255].to_vec().into(),
		format: peniko::ImageFormat::Rgba8,
		alpha_type: peniko::ImageAlphaType::Alpha,
		width: 2,
		height: 1,
	})
	.with_extend(peniko::Extend::Pad);
	let origin = Point::new(1234.5678, -987.6543);
	let target = origin + Vec2::new(1.25, 0.5);

	for zoom in [1e6, 1e8, 1e10] {
		let view = deep_view(zoom, 0., target, Point::new(128., 128.)) * Affine::translate(origin.to_vec2());
		let Some(pixels) = render(size, |scene| scene.draw_image(&image, view)) else { return };

		for x in [0, 128, 255] {
			let red = pixel(&pixels, size, x, 128)[0];
			assert!(red.abs_diff(64) <= 2, "the image at zoom {zoom} blends to red {red} rather than 64 at pixel {x}");
		}
	}
}

#[test]
fn nan_points_render_nothing_wrong() {
	let size = UVec2::new(256, 256);
	let mut path = BezPath::new();
	path.move_to((10., 10.));
	path.line_to((f64::NAN, 100.));
	path.line_to((100., 100.));
	path.close_path();
	let draw = |scene: &mut VelloSceneBuilder| {
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &path);
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(200., 200., 250., 250.));
	};

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 225, 225)[3], 255);
}

#[test]
fn many_gradients_render() {
	let size = UVec2::new(1024, 1024);
	let draw = |scene: &mut VelloSceneBuilder| {
		for i in 0..250_000_u32 {
			let (x, y) = ((i % 500) as f64 * 2., (i / 500) as f64 * 2.);
			let gradient = peniko::Gradient::new_linear((x, y), (x + 2., y)).with_stops([Color::from_rgb8((i % 256) as u8, 0, 0), Color::from_rgb8(0, (i / 256 % 256) as u8, 255)]);
			scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &Rect::new(x, y, x + 2., y + 2.));
		}
	};

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 500, 500)[3], 255);
}

#[test]
fn deep_layer_nesting_renders() {
	let size = UVec2::new(256, 256);
	let draw = |scene: &mut VelloSceneBuilder| {
		for _ in 0..2000 {
			scene.push_layer(Fill::NonZero, peniko::Mix::Multiply, 1., Affine::IDENTITY, &Rect::new(0., 0., 256., 256.));
		}
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(0., 0., 256., 256.));
		for _ in 0..2000 {
			scene.pop_layer();
		}
	};

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 128, 128)[3], 255);
}

#[test]
fn render_larger_than_the_texture_limit_is_refused() {
	let Some(executor) = executor() else { return };
	let limit = executor.context().device.limits().max_texture_dimension_2d;
	let size = UVec2::new(limit + 100, 16);
	let texture = executor.render_vello(size, DAffine2::IDENTITY, &mut |scene| {
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(0., 0., 1e6, 1e6))
	});
	assert!(texture.is_err());
}

#[test]
fn layer_four_times_the_layer_limit_renders_across_nested_splits() {
	let Some(executor) = executor() else { return };
	let quarter = crate::MAX_LAYER_RENDER_SIZE;
	if executor.context().device.limits().max_texture_dimension_2d < 4 * quarter {
		return;
	}

	// The layer spans the whole render, so it only fits once the render is halved twice
	let size = UVec2::new(4 * quarter, 8);
	let boundary = quarter * 3 / 2;
	let draw = |scene: &mut VelloSceneBuilder| {
		scene.push_layer(Fill::NonZero, peniko::Mix::Normal, 0.5, Affine::IDENTITY, &Rect::new(0., 0., size.x as f64, 8.));
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(0., 0., boundary as f64, 8.));
		scene.fill(Fill::NonZero, Affine::IDENTITY, BLUE, None, &Rect::new(boundary as f64, 0., size.x as f64, 8.));
		scene.pop_layer();
	};

	// Each side of the seams between quarters, and of the color boundary, which must land where it was drawn
	let Some(pixels) = render(size, draw) else { return };
	for x in [quarter - 1, quarter, boundary - 1, boundary, 2 * quarter - 1, 2 * quarter, 3 * quarter - 1, 3 * quarter] {
		let [r, g, b, a] = pixel(&pixels, size, x, 4);
		let expected = if x < boundary { [255, 0, 0] } else { [0, 0, 255] };
		assert_eq!([r, g, b], expected, "color at x = {x}");
		assert!(a.abs_diff(128) <= 1, "alpha {a} at x = {x}");
	}
}

#[test]
fn huge_image_renders() {
	let size = UVec2::new(256, 256);
	let (width, height) = (70_000_u32, 4_u32);
	let data: Vec<u8> = (0..width * height).flat_map(|_| [255, 0, 0, 255]).collect();
	let image = peniko::ImageBrush::new(peniko::ImageData {
		data: data.into(),
		format: peniko::ImageFormat::Rgba8,
		alpha_type: peniko::ImageAlphaType::Alpha,
		width,
		height,
	});
	let draw = |scene: &mut VelloSceneBuilder| scene.draw_image(&image, Affine::scale_non_uniform(256. / width as f64, 64.));

	let Some(pixels) = render(size, draw) else { return };
	assert_eq!(pixel(&pixels, size, 128, 128)[3], 255);
}

#[test]
fn degenerate_strokes_render() {
	let size = UVec2::new(256, 256);
	let draw = |scene: &mut VelloSceneBuilder| {
		scene.stroke(&Stroke::new(1e20), Affine::IDENTITY, RED, None, &Line::new((10., 10.), (20., 20.)));
		scene.stroke(&Stroke::new(f64::NAN), Affine::IDENTITY, RED, None, &Line::new((10., 10.), (20., 20.)));
		scene.stroke(&Stroke::new(5.), Affine::scale(0.), RED, None, &Line::new((10., 10.), (20., 20.)));
		scene.stroke(&Stroke::new(5.), Affine::scale(f64::INFINITY), RED, None, &Line::new((10., 10.), (20., 20.)));
		scene.stroke(&Stroke::new(5.), Affine::IDENTITY, RED, None, &Line::new((50., 50.), (50., 50.)));
	};

	let Some(_) = render(size, draw) else { return };
}

/// Renders an opaque white square from (`min`, `min`) to (`max`, `max`) through `effects` with `fill`.
fn render_square_with_effects(effects: Vec<graphic_types::EffectOperation>, fill: f32, min: f64, max: f64, size: UVec2) -> Option<Vec<[u8; 4]>> {
	render_square_with_effects_in(effects, DAffine2::IDENTITY, fill, min, max, size)
}

/// Like [`render_square_with_effects`], carrying out the effects in an `effect_space` placed in the document.
fn render_square_with_effects_in(effects: Vec<graphic_types::EffectOperation>, effect_space: DAffine2, fill: f32, min: f64, max: f64, size: UVec2) -> Option<Vec<[u8; 4]>> {
	let square = Rect::new(min, min, max, max);
	let effects = graphic_types::Effects::from_operations(effects);

	render(size, |scene| {
		scene.draw_effects(&effects, fill, Some(square), effect_space, &mut |scene| {
			scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square)
		});
	})
}

/// A drop shadow falling at `offset` from the content, with its own `size` and `spread` percentage, knocked out by the content.
fn drop_shadow(color: core_types::Color, offset: glam::DVec2, size: f64, spread: f64) -> graphic_types::EffectOperation {
	graphic_types::EffectOperation::DropShadow {
		color,
		angle: offset.y.atan2(-offset.x).to_degrees(),
		distance: offset.length(),
		spread,
		size,
		contour: Default::default(),
		knockout: true,
	}
}

/// The standard normal cumulative distribution, from the Abramowitz and Stegun approximation of the error function, accurate to 1e-7.
fn normal_cdf(x: f64) -> f64 {
	let z = x.abs() / std::f64::consts::SQRT_2;
	let t = 1. / (1. + 0.327_591_1 * z);
	let polynomial = t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429))));
	let erf = 1. - polynomial * (-z * z).exp();

	if x >= 0. { 0.5 * (1. + erf) } else { 0.5 * (1. - erf) }
}

#[test]
fn gaussian_blur_follows_the_normal_distribution() {
	use graphic_types::EffectOperation;

	let size = UVec2::new(512, 256);
	// Small kernels run directly, and larger ones on halved textures
	for sigma in [2., 6., 20.] {
		let blur = EffectOperation::Blur {
			radius: sigma,
			box_blur: false,
			gamma: false,
		};
		// A square much larger than the blur, so across its left edge at x = 100, on a row far from its other edges, the coverage is the one-dimensional profile
		let Some(pixels) = render_square_with_effects(vec![blur], 1., 100., 400., size) else { return };

		let mut worst = 0_f64;
		for x in 40..160 {
			let expected = normal_cdf((x as f64 + 0.5 - 100.) / sigma);
			let actual = pixel(&pixels, size, x, 200)[3] as f64 / 255.;
			worst = worst.max((expected - actual).abs());
		}
		assert!(worst < 0.03, "σ {sigma} strays from the normal distribution by {worst}");
	}
}

#[test]
fn blurs_stretch_with_an_effect_space_scaled_unevenly() {
	use graphic_types::EffectOperation;

	// Three times as wide as it is tall, the effect space renders in a texture of its own, so the blur of 4 spreads 12 pixels across and 4 down
	let size = UVec2::new(512, 512);
	let blur = EffectOperation::Blur {
		radius: 4.,
		box_blur: false,
		gamma: false,
	};
	let Some(pixels) = render_square_with_effects_in(vec![blur], DAffine2::from_scale(glam::DVec2::new(3., 1.)), 1., 100., 400., size) else {
		return;
	};

	let stray = |sigma: f64, coverage: &dyn Fn(u32) -> u8| (40..160).map(|at| (normal_cdf((at as f64 + 0.5 - 100.) / sigma) - coverage(at) as f64 / 255.).abs()).fold(0., f64::max);
	let across = stray(12., &|x| pixel(&pixels, size, x, 250)[3]);
	let down = stray(4., &|y| pixel(&pixels, size, 250, y)[3]);
	assert!(across < 0.04 && down < 0.04, "the blur strays from σ 12 across by {across} and σ 4 down by {down}");
}

#[test]
fn shadows_turn_with_their_effect_space() {
	use core_types::Color as GraphicColor;

	// A shadow cast to the right of the content in an effect space turned a quarter clockwise falls below it in the document
	let size = UVec2::new(256, 256);
	let shadow = drop_shadow(GraphicColor::BLACK, glam::DVec2::new(30., 0.), 0., 0.);
	let Some(pixels) = render_square_with_effects_in(vec![shadow], DAffine2::from_angle(std::f64::consts::FRAC_PI_2), 1., 50., 150., size) else {
		return;
	};

	assert!(pixel(&pixels, size, 100, 165)[3] > 200, "the shadow should fall below the square");
	assert_eq!(pixel(&pixels, size, 165, 100)[3], 0, "the shadow shouldn't fall to its right");
}

#[test]
fn shadows_and_glows_fall_where_they_should() {
	use core_types::Color as GraphicColor;
	use glam::DVec2;
	use graphic_types::EffectOperation;

	let size = UVec2::new(128, 128);
	let black = GraphicColor::BLACK;

	// A hard drop shadow offset down and right shows past the square's bottom right corner, beneath it
	let shadow = drop_shadow(black, DVec2::new(10., 10.), 0., 0.);
	let Some(pixels) = render_square_with_effects(vec![shadow.clone()], 1., 30., 80., size) else {
		return;
	};
	assert_eq!(pixel(&pixels, size, 50, 50), [255, 255, 255, 255], "the content covers its shadow");
	assert_eq!(pixel(&pixels, size, 85, 85), [0, 0, 0, 255], "the shadow shows past the content");
	assert_eq!(pixel(&pixels, size, 25, 25)[3], 0, "no shadow before the offset");

	// With no fill, the content vanishes and its shadow stays knocked out where the content would be
	let Some(pixels) = render_square_with_effects(vec![shadow], 0., 30., 80., size) else { return };
	assert_eq!(pixel(&pixels, size, 50, 50)[3], 0, "the shadow is knocked out by the faded content");
	assert_eq!(pixel(&pixels, size, 85, 85), [0, 0, 0, 255], "the shadow stays at full strength");

	// An inner shadow darkens the content's top left edges from inside and stays within it
	let inner_shadow = EffectOperation::InnerShadow {
		color: black,
		angle: 135.,
		distance: 6. * std::f64::consts::SQRT_2,
		choke: 0.,
		size: 0.,
		contour: Default::default(),
	};
	let Some(pixels) = render_square_with_effects(vec![inner_shadow], 1., 30., 80., size) else {
		return;
	};
	assert_eq!(pixel(&pixels, size, 32, 50), [0, 0, 0, 255], "inside the left edge");
	assert_eq!(pixel(&pixels, size, 50, 50), [255, 255, 255, 255], "deep inside");
	assert_eq!(pixel(&pixels, size, 78, 50), [255, 255, 255, 255], "inside the right edge, away from the offset");
	assert_eq!(pixel(&pixels, size, 25, 50)[3], 0, "outside");

	// A glow grown by its whole size surrounds the content evenly
	let glow = EffectOperation::OuterGlow {
		color: GraphicColor::from_rgbaf32_unchecked(1., 0., 0., 1.).into(),
		technique: Default::default(),
		spread: 100.,
		size: 5.,
		range: 50.,
		contour: Default::default(),
	};
	let Some(pixels) = render_square_with_effects(vec![glow], 1., 30., 80., size) else { return };
	for (x, y) in [(27, 55), (82, 55), (55, 27), (55, 82)] {
		assert_eq!(pixel(&pixels, size, x, y), [255, 0, 0, 255], "glow at ({x}, {y})");
	}
	for (x, y) in [(23, 55), (87, 55)] {
		assert_eq!(pixel(&pixels, size, x, y)[3], 0, "past the glow at ({x}, {y})");
	}
}

#[test]
fn later_shadows_stack_over_earlier_ones_beneath_the_content() {
	use core_types::Color as GraphicColor;
	use glam::DVec2;

	let size = UVec2::new(128, 128);
	let shadow = |color, offset: f64| drop_shadow(color, DVec2::splat(offset), 0., 0.);

	// Both shadows are cast from the content rather than from each other, and the later one is on top where they overlap
	let effects = vec![shadow(GraphicColor::BLACK, 10.), shadow(GraphicColor::from_rgbaf32_unchecked(0., 0., 1., 1.), 5.)];
	let Some(pixels) = render_square_with_effects(effects, 1., 30., 80., size) else { return };
	assert_eq!(pixel(&pixels, size, 82, 82), [0, 0, 255, 255], "the later shadow is on top");
	assert_eq!(pixel(&pixels, size, 88, 88), [0, 0, 0, 255], "the earlier shadow reaches further");
	assert_eq!(pixel(&pixels, size, 93, 93)[3], 0, "no shadow of a shadow");
}

#[test]
fn nested_effects_groups_render_inside_out() {
	use core_types::Color as GraphicColor;
	use glam::DVec2;
	use graphic_types::{EffectOperation, Effects};

	let size = UVec2::new(128, 128);
	let square = Rect::new(30., 30., 80., 80.);

	// A hard shadow around a group with content that is itself a blurred square
	let shadow = Effects::from_operations([drop_shadow(GraphicColor::BLACK, DVec2::new(20., 0.), 0., 0.)]);
	let blur = Effects::from_operations([EffectOperation::Blur {
		radius: 3.,
		box_blur: false,
		gamma: false,
	}]);
	let draw = |scene: &mut VelloSceneBuilder| {
		scene.draw_effects(&shadow, 1., Some(square.inflate(12., 12.)), DAffine2::IDENTITY, &mut |scene| {
			scene.draw_effects(&blur, 1., Some(square), DAffine2::IDENTITY, &mut |scene| {
				scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square)
			});
		});
	};

	let Some(pixels) = render(size, draw) else { return };
	let edge = pixel(&pixels, size, 30, 55);
	assert!((100..=155).contains(&edge[3]), "the blur softens the edge to half coverage, not {edge:?}");
	// Ten pixels past the content's blurred edge, where the shadow is cast from ten pixels inside it
	let shadow = pixel(&pixels, size, 90, 55);
	assert!(shadow[3] >= 250 && shadow[0] <= 5, "the shadow is cast by the blurred group, not {shadow:?}");
}

#[test]
fn cpu_evaluated_effects_match_the_gpu_passes() {
	use core_types::Color as GraphicColor;
	use core_types::list::Item;
	use core_types::transfer_curve::TransferCurve;
	use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM};
	use glam::DVec2;
	use graphic_types::adjustment::{Adjustment, LevelsRecord, LevelsSettings};
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::vector_types::gradient::{Gradient, GradientStop};
	use graphic_types::{Effect, EffectOperation, GlowTechnique, Graphic, stamp_effect};
	use raster_kernels::evaluate_raster_effects;

	let size = UVec2::new(160, 160);
	let (min, max) = (50, 100);
	let shadow_color = GraphicColor::from_rgbaf32_unchecked(0.1, 0.2, 0.8, 0.75);
	let blur = |radius, box_blur, gamma| EffectOperation::Blur { radius, box_blur, gamma };
	let identity = LevelsRecord {
		shadows: 0.,
		midtones: 1.,
		highlights: 100.,
		output_minimums: 0.,
		output_maximums: 100.,
	};

	let gradient_glow = |color, technique, spread, size, range, contour| EffectOperation::OuterGlow {
		color,
		technique,
		spread,
		size,
		range,
		contour,
	};
	let glow = |technique, spread, size, range, contour| gradient_glow(shadow_color.into(), technique, spread, size, range, contour);
	let stop = |position, color| GradientStop { position, midpoint: 0.5, color };
	let fading = Graphic::from(Gradient::new([
		stop(0., GraphicColor::from_rgbaf32_unchecked(1., 0.9, 0.2, 1.)),
		stop(0.4, GraphicColor::from_rgbaf32_unchecked(0.9, 0.2, 0.1, 0.8)),
		stop(1., GraphicColor::from_rgbaf32_unchecked(0.2, 0., 0.6, 0.)),
	]));
	let opaque = Graphic::from(Gradient::new([
		stop(0., GraphicColor::from_rgbaf32_unchecked(0.1, 0.8, 0.3, 1.)),
		stop(1., GraphicColor::from_rgbaf32_unchecked(0.2, 0.1, 0.9, 1.)),
	]));
	let bow = TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.25, 0.75), DVec2::ONE]);

	let cases = [
		vec![blur(3., false, false)],
		vec![blur(20., false, true)],
		vec![blur(5.5, true, false)],
		vec![drop_shadow(shadow_color, DVec2::new(6.5, -3.), 8., 25.)],
		vec![drop_shadow(shadow_color, DVec2::new(2., 3.), 40., 0.)],
		vec![EffectOperation::InnerShadow {
			color: shadow_color,
			angle: 135.,
			distance: 5.,
			choke: 50.,
			size: 6.,
			contour: TransferCurve::default(),
		}],
		vec![glow(GlowTechnique::Softer, 30., 9., 50., TransferCurve::default()), blur(2., false, false)],
		vec![glow(GlowTechnique::Precise, 0., 12., 75., bow.clone())],
		vec![glow(GlowTechnique::Precise, 50., 16., 100., TransferCurve::default())],
		vec![glow(GlowTechnique::Precise, 80., 10., 100., TransferCurve::default())],
		vec![glow(GlowTechnique::Softer, 0., 6., 50., bow.clone())],
		vec![gradient_glow(fading, GlowTechnique::Precise, 0., 14., 75., TransferCurve::default())],
		vec![gradient_glow(opaque, GlowTechnique::Softer, 25., 12., 50., bow)],
		// Bends alpha, so the fill must fade the content before it on both
		vec![EffectOperation::Adjustment(Adjustment::Levels(Box::new(LevelsSettings {
			composite: identity,
			red: identity,
			green: identity,
			blue: identity,
			alpha: LevelsRecord { midtones: 2.2, ..identity },
		})))],
	];

	for effects in cases {
		for fill in [1., 0.5] {
			let Some(gpu) = render_square_with_effects(effects.clone(), fill, min as f64, max as f64, size) else {
				return;
			};

			// The same square as a raster covering the render one pixel to one pixel
			let mut image = Image::new(size.x, size.y, GraphicColor::TRANSPARENT);
			for y in min..max {
				for x in min..max {
					image.data[(y * size.x + x) as usize] = GraphicColor::WHITE;
				}
			}
			let mut item = Item::new_from_element(Raster::new_cpu(image))
				.with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(size.as_dvec2()))
				.with_attribute(ATTR_OPACITY_FILL, fill as f64);
			effects.iter().for_each(|operation| stamp_effect(&mut item, Effect::new(operation)));
			let evaluated = evaluate_raster_effects(item);

			// Reads the evaluated image at the center of each render pixel through its grown placement
			let placement: DAffine2 = evaluated.attribute_cloned_or_default(ATTR_TRANSFORM);
			let raster = evaluated.element();
			let document_to_pixels = DAffine2::from_scale(DVec2::new(raster.width as f64, raster.height as f64)) * placement.inverse();

			let mut worst = 0;
			for y in 0..size.y {
				for x in 0..size.x {
					let point = document_to_pixels.transform_point2(DVec2::new(x as f64 + 0.5, y as f64 + 0.5)).floor();
					let cpu = raster.data[(point.y as u32 * raster.width + point.x as u32) as usize];
					let [red, green, blue, alpha] = cpu.to_gamma_srgb_channels();
					let cpu = [red * alpha, green * alpha, blue * alpha, alpha].map(|channel| channel * 255.);

					// Compared premultiplied, so a nearly transparent pixel's color can't dominate
					let gpu = pixel(&gpu, size, x, y);
					let gpu_alpha = gpu[3] as f32 / 255.;
					for channel in 0..4 {
						let gpu_channel = if channel == 3 { gpu[3] as f32 } else { gpu[channel] as f32 * gpu_alpha };
						worst = worst.max((cpu[channel] - gpu_channel).abs().round() as u32);
					}
				}
			}
			assert!(worst <= 2, "{effects:?} at fill {fill} differs from the GPU by up to {worst} levels");
		}
	}
}

/// Colors across the RGB cube, among them black, white, grays, and saturated and near-black colors, each opaque, with some also translucent or fully transparent.
fn adjustment_test_colors() -> Vec<core_types::Color> {
	const LEVELS: [f32; 12] = [0., 1. / 255., 4. / 255., 0.05, 0.2, 0.35, 0.5, 0.65, 0.8, 0.95, 254. / 255., 1.];
	let count = LEVELS.len();

	let mut colors = Vec::new();
	for index in 0..count * count * count {
		let [r, g, b] = [index / (count * count), index / count % count, index % count].map(|level| LEVELS[level]);
		colors.push(core_types::Color::from_gamma_srgb_channels(r, g, b, 1.));

		for (spacing, alpha) in [(5, 0.5), (7, 0.15), (97, 0.)] {
			if index % spacing == 0 {
				colors.push(core_types::Color::from_gamma_srgb_channels(r, g, b, alpha));
			}
		}
	}
	colors
}

/// What the GPU's adjustment passes carrying out `run` at `fill` leave of each of `texels`, which like the result hold premultiplied gamma-encoded channels
/// as the half floats of the working format.
fn gpu_adjusted_texels(run: &[graphic_types::Adjustment], texels: &[[half::f16; 4]], fill: f32) -> Option<Vec<[f32; 4]>> {
	use graphic_types::{EffectOperation, Effects};
	use half::f16;
	use wgpu::util::DeviceExt;

	let executor = executor()?;
	let effect_passes = &executor.inner.vello_renderer.effect_passes;
	if !effect_passes.renders_working_formats {
		return None;
	}
	let device = &executor.context().device;
	let _turn = take_turn();

	// One texel for each pixel
	let width = 64;
	let height = texels.len().div_ceil(width);
	let mut padded = texels.to_vec();
	padded.resize(width * height, [f16::ZERO; 4]);
	let descriptor = wgpu::TextureDescriptor {
		label: Some("adjustment_test_colors"),
		size: wgpu::Extent3d {
			width: width as u32,
			height: height as u32,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba16Float,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		view_formats: &[],
	};

	let queue = executor.context().queue.lock();
	let content = device.create_texture_with_data(&queue, &descriptor, wgpu::util::TextureDataOrder::LayerMajor, bytemuck::cast_slice(&padded));
	let effects = Effects::from_operations(run.iter().cloned().map(EffectOperation::Adjustment));
	let mut textures = crate::vello_effects::EffectTextures::default();
	let adjusted = effect_passes.apply(device, &queue, &mut textures, &content, &effects, fill, glam::DMat2::IDENTITY, None);
	drop(queue);

	let bytes = executor.read_texture_pixels(&adjusted).expect("Failed to read back the adjusted colors");
	let half_floats: Vec<f32> = bytes.as_chunks::<2>().0.iter().map(|&bytes| f16::from_le_bytes(bytes).to_f32()).collect();
	Some(half_floats.as_chunks::<4>().0[..texels.len()].to_vec())
}

/// How far, in 8-bit levels of the straight gamma-encoded channels and alpha (scaled down for values past 1 by their size, as half floats round them),
/// the GPU's adjustment passes carrying out `run` on `colors` at `fill` stray from the CPU's kernels, with the pixel that strays the most.
/// The colors reach the GPU as the half floats of its working format, so the CPU adjusts them as the GPU reads them.
fn gpu_adjustment_deviation(run: &[graphic_types::Adjustment], colors: &[core_types::Color], fill: f32) -> Option<(f32, String)> {
	use half::f16;

	// One color for each pixel, premultiplied with gamma-encoded channels as the effect passes hold them
	let texels: Vec<[f16; 4]> = (colors.iter())
		.map(|color| {
			let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
			[red * alpha, green * alpha, blue * alpha, alpha].map(f16::from_f32)
		})
		.collect();
	let gpu = gpu_adjusted_texels(run, &texels, fill)?;

	// The texels as the adjustments read them (which a fill fades first, into half floats again), adjusted one after another as the CPU carries out effects
	let inputs: Vec<[f32; 4]> = texels.iter().map(|texel| texel.map(|channel| f16::from_f32(channel.to_f32() * fill).to_f32())).collect();
	let mut cpu = inputs.clone();
	run.iter().for_each(|adjustment| raster_kernels::raster_effects::adjust_premultiplied(&mut cpu, adjustment));

	let straight = |[red, green, blue, alpha]: [f32; 4]| if alpha > 0. { [red / alpha, green / alpha, blue / alpha, alpha] } else { [0.; 4] };
	let mut worst = (0_f32, String::new());
	for (index, (input, cpu)) in inputs.iter().zip(&cpu).enumerate() {
		let (input, gpu, cpu) = (straight(*input), straight(gpu[index]), straight(*cpu));

		// Without alpha a pixel shows no color to compare, and a NaN on only one side strays without bound
		let channels = if gpu[3] > 0. && cpu[3] > 0. { 0..4 } else { 3..4 };
		for channel in channels {
			let deviation = match (gpu[channel].is_nan(), cpu[channel].is_nan()) {
				(false, false) => (gpu[channel] - cpu[channel]).abs() * 255. / cpu[channel].abs().max(1.),
				(true, true) => 0.,
				_ => f32::INFINITY,
			};
			if deviation > worst.0 {
				worst = (deviation, format!("{input:?} as {gpu:?} rather than {cpu:?}"));
			}
		}
	}
	Some(worst)
}

/// Checks that each named run of adjustments strays from the CPU's kernels on the GPU by at most the rounding of its half floats, printing how far each strays.
fn assert_gpu_adjustments_match_the_cpu_kernels(runs: Vec<(impl std::fmt::Display, Vec<graphic_types::Adjustment>)>) {
	assert_gpu_adjustments_stray_at_most(0.3, runs, &[1.]);
}

/// Checks that each named run strays from the CPU's kernels by at most `limit` levels at each of `fills`, which should be powers of two so fading by them rounds nothing.
fn assert_gpu_adjustments_stray_at_most(limit: f32, runs: Vec<(impl std::fmt::Display, Vec<graphic_types::Adjustment>)>, fills: &[f32]) {
	let colors = adjustment_test_colors();

	let mut strays = Vec::new();
	for (name, run) in runs {
		for &fill in fills {
			let Some((levels, worst_pixel)) = gpu_adjustment_deviation(&run, &colors, fill) else { return };
			println!("{name} at a fill of {fill}: {levels:.3} levels");

			if levels > limit {
				strays.push(format!("{name} at a fill of {fill} strays from the CPU's kernels by {levels} levels, adjusting {worst_pixel}"));
			}
		}
	}
	assert!(strays.is_empty(), "{}", strays.join("\n"));
}

#[test]
fn gpu_tone_adjustments_match_the_cpu_kernels() {
	use core_types::transfer_curve::TransferCurve;
	use glam::DVec2;
	use graphic_types::adjustment::*;

	let record = |shadows, midtones, highlights, output_minimums, output_maximums| LevelsRecord {
		shadows,
		midtones,
		highlights,
		output_minimums,
		output_maximums,
	};
	let identity = record(0., 1., 100., 0., 100.);
	let levels = |composite, red, green, blue, alpha| Adjustment::Levels(Box::new(LevelsSettings { composite, red, green, blue, alpha }));

	let curve = |points: &[(f64, f64)]| TransferCurve::new(points.iter().map(|&(x, y)| DVec2::new(x, y)).collect());
	let curves = |composite, red, green, blue, alpha| Adjustment::Curves(Box::new(CurvesSettings { composite, red, green, blue, alpha }));
	let straight = TransferCurve::default;

	let brightness_contrast = |brightness, contrast, use_classic| Adjustment::BrightnessContrast {
		brightness,
		contrast,
		use_classic,
		classic_pivot: 127.,
	};
	let exposure = |exposure, offset, gamma_correction| Adjustment::Exposure { exposure, offset, gamma_correction };

	let runs = [
		("Levels", levels(record(10., 1.4, 90., 0., 100.), identity, record(5., 0.8, 100., 0., 100.), identity, identity)),
		(
			"Levels with midtones at their bounds",
			levels(record(0., 9.99, 100., 0., 100.), record(0., 0.01, 100., 0., 100.), identity, record(20., 0.1, 80., 10., 90.), identity),
		),
		(
			"Levels with gammas merged into one",
			levels(record(0., 2.5, 100., 5., 95.), record(30., 3.2, 100., 0., 100.), record(0., 0.3, 60., 0., 100.), identity, identity),
		),
		(
			"Levels with inverted output and a bent alpha",
			levels(record(40., 1., 41., 90., 10.), identity, identity, identity, record(10., 2.2, 90., 0., 80.)),
		),
		(
			"Curves",
			curves(
				curve(&[(0., 0.), (0.25, 0.4), (0.75, 0.7), (1., 1.)]),
				straight(),
				curve(&[(0., 0.1), (1., 0.9)]),
				straight(),
				straight(),
			),
		),
		(
			"Curves with steep segments",
			curves(
				curve(&[(0., 0.), (0.45, 0.02), (0.5, 0.98), (1., 1.)]),
				curve(&[(0.2, 0.9), (0.21, 0.1)]),
				curve(&[(0.5, 0.5)]),
				TransferCurve::new(Vec::new()),
				curve(&[(0., 1.), (1., 0.)]),
			),
		),
		("Brightness/Contrast", brightness_contrast(40., 30., false)),
		("Brightness/Contrast darkening", brightness_contrast(-60., -40., false)),
		("Brightness/Contrast beyond 100", brightness_contrast(150., 100., false)),
		("Brightness/Contrast beyond -100", brightness_contrast(-150., -50., false)),
		("Brightness/Contrast classic", brightness_contrast(50., 50., true)),
		("Brightness/Contrast classic squeezed", brightness_contrast(-30., -100., true)),
		("Brightness/Contrast classic at full contrast", brightness_contrast(20., 100., true)),
		("Exposure", exposure(1.5, 0.05, 1.2)),
		("Exposure darkening", exposure(-2., -0.1, 0.5)),
		("Exposure at its extremes", exposure(8., 0., 9.99)),
		("Exposure with the least gamma", exposure(0., 0.3, 0.01)),
		("Gamma Correction", Adjustment::GammaCorrection { gamma: 2.2, inverse: false }),
		("Gamma Correction inverse", Adjustment::GammaCorrection { gamma: 0.4, inverse: true }),
		("Gamma Correction at its extremes", Adjustment::GammaCorrection { gamma: 0.01, inverse: false }),
		("Posterize to 2 levels", Adjustment::Posterize { levels: 2 }),
		("Posterize to 7 levels", Adjustment::Posterize { levels: 7 }),
		("Posterize to 255 levels", Adjustment::Posterize { levels: 255 }),
		(
			"Threshold",
			Adjustment::Threshold {
				min_luminance: 37.3,
				max_luminance: 88.1,
			},
		),
		(
			"Threshold across everything",
			Adjustment::Threshold {
				min_luminance: 0.,
				max_luminance: 100.,
			},
		),
		(
			"Threshold of the shadows",
			Adjustment::Threshold {
				min_luminance: 12.3,
				max_luminance: 47.9,
			},
		),
		("Invert", Adjustment::Invert),
		("Make Opaque", Adjustment::MakeOpaque),
	];
	assert_gpu_adjustments_match_the_cpu_kernels(runs.into_iter().map(|(name, adjustment)| (name, vec![adjustment])).collect());
}

#[test]
fn gpu_color_adjustments_match_the_cpu_kernels() {
	use core_types::Color;
	use core_types::blending::BlendMode;
	use graphic_types::adjustment::*;

	let desaturate = |method| Adjustment::Desaturate { method };

	let black_and_white = |weights: [f64; 6], use_tint| {
		let [reds, yellows, greens, cyans, blues, magentas] = weights;
		Adjustment::BlackAndWhite(BlackAndWhiteSettings {
			use_tint,
			tint: Color::from_gamma_srgb_channels(0.88, 0.83, 0.7, 1.),
			reds,
			yellows,
			greens,
			cyans,
			blues,
			magentas,
		})
	};

	let sliders = |hue, saturation, lightness| HueSaturationSliders { hue, saturation, lightness };
	let range = |sliders, [falloff_start, range_start, range_end, falloff_end]: [f64; 4]| HueRangeSettings {
		sliders,
		falloff_start,
		range_start,
		range_end,
		falloff_end,
	};
	let zero = sliders(0., 0., 0.);
	let hue_saturation = |master, colorize, ranges: [HueSaturationSliders; 6]| {
		let [reds, yellows, greens, cyans, blues, magentas] = ranges;
		Adjustment::HueSaturation(Box::new(HueSaturationSettings {
			master,
			colorize,
			colorize_sliders: sliders(200., 60., -30.),
			reds: range(reds, [315., 345., 15., 45.]),
			yellows: range(yellows, [15., 45., 75., 105.]),
			greens: range(greens, [75., 105., 135., 165.]),
			cyans: range(cyans, [135., 165., 195., 225.]),
			blues: range(blues, [195., 225., 255., 285.]),
			magentas: range(magentas, [255., 285., 315., 345.]),
		}))
	};

	let row = |red, green, blue, constant| ChannelMixerRow { red, green, blue, constant };
	let channel_mixer = |monochrome, red, green, blue| {
		Adjustment::ChannelMixer(Box::new(ChannelMixerSettings {
			monochrome,
			monochrome_mix: row(40., 40., 20., 5.),
			red,
			green,
			blue,
		}))
	};

	let inks = |cyan, magenta, yellow, black| SelectiveColorInks { cyan, magenta, yellow, black };
	let selective_color = |mode, groups: [SelectiveColorInks; 9]| {
		let [reds, yellows, greens, cyans, blues, magentas, whites, neutrals, blacks] = groups;
		Adjustment::SelectiveColor(Box::new(SelectiveColorSettings {
			mode,
			reds,
			yellows,
			greens,
			cyans,
			blues,
			magentas,
			whites,
			neutrals,
			blacks,
		}))
	};
	let some_inks = [
		inks(-20., 15., 30., 0.),
		inks(0., 0., 0., 0.),
		inks(35., -10., 0., 20.),
		inks(-40., 25., 10., -15.),
		inks(10., 10., -30., 5.),
		inks(0., -50., 45., 0.),
		inks(-25., 0., 0., 30.),
		inks(15., -15., 20., -10.),
		inks(0., 20., -20., 40.),
	];
	let extreme_inks = [100., -100., 100., -100., 100., -100., 100., -100., 100.].map(|percent| inks(percent, -percent, percent, -percent));

	let shifts = |cyan_red, magenta_green, yellow_blue| ColorBalanceShifts { cyan_red, magenta_green, yellow_blue };
	let color_balance = |shadows, midtones, highlights, preserve_luminosity| {
		Adjustment::ColorBalance(ColorBalanceSettings {
			shadows,
			midtones,
			highlights,
			preserve_luminosity,
		})
	};

	let photo_filter = |color, density, preserve_luminosity| Adjustment::PhotoFilter { color, density, preserve_luminosity };
	let warming = Color::from_gamma_srgb_channels(0.925, 0.541, 0., 1.);
	let cooling = Color::from_gamma_srgb_channels(0., 0.42, 1., 1.);

	let runs = [
		("Desaturate by Rec. 709 luminance", desaturate(DesaturateMethod::LuminanceRec709)),
		("Desaturate by Rec. 709 luma", desaturate(DesaturateMethod::LumaRec709)),
		("Desaturate by Rec. 601 luma", desaturate(DesaturateMethod::LumaRec601)),
		("Desaturate by OkLab lightness", desaturate(DesaturateMethod::LightnessOkLab)),
		("Desaturate by the channels' average", desaturate(DesaturateMethod::ChannelsAverage)),
		("Desaturate by the channels' minimum", desaturate(DesaturateMethod::ChannelsMinimum)),
		("Desaturate by the channels' maximum", desaturate(DesaturateMethod::ChannelsMaximum)),
		("Desaturate by HSL lightness", desaturate(DesaturateMethod::LightnessHsl)),
		("Black & White", black_and_white([40., 60., 40., 60., 20., 80.], false)),
		("Black & White tinted at extreme weights", black_and_white([300., -200., 150., -50., 250., -100.], true)),
		("Hue/Saturation", hue_saturation(sliders(25., 30., 5.), false, [sliders(-15., 40., 0.), zero, zero, zero, zero, zero])),
		(
			"Hue/Saturation by range at extremes",
			hue_saturation(
				sliders(-120., -20., 0.),
				false,
				[zero, sliders(30., -100., 50.), sliders(0., 100., 0.), zero, sliders(60., 0., -100.), sliders(-90., 60., 100.)],
			),
		),
		("Hue/Saturation fully saturating", hue_saturation(sliders(0., 100., -40.), false, [zero; 6])),
		// A range's full lightness leaves grays, which the full saturation of another slider has no color in to raise
		(
			"Hue/Saturation with a range's lightness and the master's saturation at their ends",
			hue_saturation(sliders(0., 100., 0.), false, [sliders(0., 0., -100.), zero, sliders(0., 0., 100.), zero, zero, zero]),
		),
		(
			"Hue/Saturation with a range's lightness and its own saturation at their ends",
			hue_saturation(zero, false, [sliders(0., 100., 100.), zero, zero, sliders(0., 100., -100.), zero, zero]),
		),
		(
			"Hue/Saturation with the master's lightness and saturation at their ends",
			hue_saturation(sliders(0., 100., 100.), false, [zero; 6]),
		),
		("Hue/Saturation fully desaturating and lightening", hue_saturation(sliders(180., -100., 100.), false, [zero; 6])),
		("Hue/Saturation colorizing", hue_saturation(zero, true, [zero; 6])),
		("Vibrance", Adjustment::Vibrance { vibrance: 60., saturation: 20. }),
		("Vibrance draining", Adjustment::Vibrance { vibrance: -80., saturation: -50. }),
		("Vibrance at its most", Adjustment::Vibrance { vibrance: 100., saturation: 100. }),
		("Vibrance at its least", Adjustment::Vibrance { vibrance: -100., saturation: -100. }),
		("Channel Mixer", channel_mixer(false, row(80., 20., 0., 0.), row(10., 70., 20., 0.), row(0., 30., 90., -10.))),
		(
			"Channel Mixer at extremes",
			channel_mixer(false, row(200., -200., 50., 100.), row(-150., 200., -50., -100.), row(33.3, 33.3, 33.3, 0.)),
		),
		("Channel Mixer in monochrome", channel_mixer(true, row(100., 0., 0., 0.), row(0., 100., 0., 0.), row(0., 0., 100., 0.))),
		("Selective Color", selective_color(RelativeAbsolute::Relative, some_inks)),
		("Selective Color absolute", selective_color(RelativeAbsolute::Absolute, some_inks)),
		("Selective Color at extremes", selective_color(RelativeAbsolute::Relative, extreme_inks)),
		("Selective Color absolute at extremes", selective_color(RelativeAbsolute::Absolute, extreme_inks)),
		("Color Balance", color_balance(shifts(30., -20., 10.), shifts(-40., 25., 60.), shifts(15., -35., -10.), true)),
		(
			"Color Balance without preserving luminosity",
			color_balance(shifts(30., -20., 10.), shifts(-40., 25., 60.), shifts(15., -35., -10.), false),
		),
		(
			"Color Balance at extremes",
			color_balance(shifts(100., -100., 100.), shifts(-100., 100., -100.), shifts(100., 100., -100.), false),
		),
		("Photo Filter", photo_filter(warming, 25., true)),
		("Photo Filter at full density", photo_filter(cooling, 100., false)),
		("Photo Filter at full density preserving luminosity", photo_filter(cooling, 100., true)),
	];
	let mut runs: Vec<(String, Vec<Adjustment>)> = runs.into_iter().map(|(name, adjustment)| (name.to_string(), vec![adjustment])).collect();

	// Every blend mode, overlaying an opaque color and a translucent one
	for blend_mode in BlendMode::list().into_iter().flatten().chain(&[BlendMode::Erase, BlendMode::Restore, BlendMode::MultiplyAlpha]) {
		for (color, opacity) in [(Color::from_gamma_srgb_channels(0.9, 0.45, 0.2, 1.), 70.), (Color::from_gamma_srgb_channels(0.1, 0.6, 0.85, 0.4), 100.)] {
			let overlay = Adjustment::ColorOverlay {
				color,
				blend_mode: *blend_mode,
				opacity,
			};
			runs.push((format!("Color Overlay with {blend_mode} at {opacity}% opacity"), vec![overlay]));
		}
	}

	assert_gpu_adjustments_match_the_cpu_kernels(runs);
}

#[test]
fn gpu_gradient_maps_match_the_cpu_kernels() {
	use core_types::Color;
	use core_types::list::{ATTR_GRADIENT_CYCLIC, ATTR_GRADIENT_HUE_DIRECTION, ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPACE, ATTR_GRADIENT_SPREAD, Item};
	use graphic_types::Adjustment;
	use graphic_types::vector_types::gradient::{Gradient, GradientHueDirection, GradientInterpolation, GradientSpace, GradientSpread, GradientStop};

	let stop = |position, midpoint, [red, green, blue]: [f32; 3]| GradientStop {
		position,
		midpoint,
		color: Color::from_gamma_srgb_channels(red, green, blue, 1.),
	};
	let stops = vec![stop(0., 0.3, [0.05, 0., 0.3]), stop(0.35, 0.5, [0.9, 0.2, 0.1]), stop(1., 0.75, [1., 0.95, 0.4])];
	let gradient_map = |stops: &Vec<GradientStop>, space, interpolation, hue_direction, spread, cyclic, reverse| Adjustment::GradientMap {
		gradient: Item::new_from_element(Gradient::new(stops.iter().copied()))
			.with_attribute(ATTR_GRADIENT_SPACE, space)
			.with_attribute(ATTR_GRADIENT_INTERPOLATION, interpolation)
			.with_attribute(ATTR_GRADIENT_HUE_DIRECTION, hue_direction)
			.with_attribute(ATTR_GRADIENT_SPREAD, spread)
			.with_attribute(ATTR_GRADIENT_CYCLIC, cyclic),
		reverse,
	};
	let spaces = [
		GradientSpace::OkLab,
		GradientSpace::Lab,
		GradientSpace::OkLCh,
		GradientSpace::LCh,
		GradientSpace::RgbLinear,
		GradientSpace::RgbGamma,
		GradientSpace::Hsv,
		GradientSpace::Hsl,
	];

	let mut runs = Vec::new();
	for space in spaces {
		for interpolation in [GradientInterpolation::Linear, GradientInterpolation::Smooth] {
			let name = format!("Gradient Map {interpolation:?} in {space:?}");
			runs.push((name, gradient_map(&stops, space, interpolation, GradientHueDirection::Shorter, GradientSpread::Pad, false, false)));
		}
	}
	for hue_direction in [GradientHueDirection::Longer, GradientHueDirection::Increasing, GradientHueDirection::Decreasing] {
		for (space, interpolation) in [(GradientSpace::OkLCh, GradientInterpolation::Linear), (GradientSpace::Hsl, GradientInterpolation::Smooth)] {
			let name = format!("Gradient Map {interpolation:?} in {space:?} the {hue_direction:?} way");
			runs.push((name, gradient_map(&stops, space, interpolation, hue_direction, GradientSpread::Pad, false, false)));
		}
	}

	// Hard steps where no color's position lands exactly, which float rounding could tip either way, then stops past the ends, a single stop, and none
	let steps = vec![stop(0.05, 0.5, [0.05, 0., 0.3]), stop(0.3721, 0.5, [0.9, 0.2, 0.1]), stop(0.9, 0.5, [1., 0.95, 0.4])];
	let wrapping = vec![stop(0.08, 0.3, [0.05, 0., 0.3]), stop(0.35, 0.5, [0.9, 0.2, 0.1]), stop(0.83, 0.65, [1., 0.95, 0.4])];
	let hard_edge = vec![
		stop(0., 0.5, [0.1, 0.1, 0.1]),
		stop(0.4137, 0.5, [0.9, 0.2, 0.1]),
		stop(0.4137, 0.5, [0.1, 0.3, 0.9]),
		stop(1., 0.5, [0.9, 0.9, 0.9]),
	];
	let outside = vec![stop(-0.2, 0.6, [0., 0.4, 0.2]), stop(0.5, 0.5, [0.7, 0.7, 0.1]), stop(1.3, 0.5, [0.2, 0., 0.9])];
	let single = vec![stop(0.4, 0.5, [0.3, 0.6, 0.9])];

	// Stops exactly where the test colors' black, grays, and white land, which each take the color of the stop they land on
	let at_grays = vec![
		stop(0., 0.5, [0.1, 0.1, 0.1]),
		stop(0.2, 0.5, [0.9, 0.2, 0.1]),
		stop(0.5, 0.5, [0.1, 0.3, 0.9]),
		stop(0.8, 0.5, [0.2, 0.8, 0.3]),
		stop(1., 0.5, [0.9, 0.9, 0.9]),
	];

	let linear = (GradientSpace::OkLab, GradientInterpolation::Linear, GradientHueDirection::Shorter);
	let stepped = (GradientSpace::OkLab, GradientInterpolation::Stepped, GradientHueDirection::Shorter);
	for (name, stops, (space, interpolation, hue_direction), spread, cyclic, reverse) in [
		("stepped", &steps, stepped, GradientSpread::Pad, false, false),
		("stepped and cyclic", &steps, stepped, GradientSpread::Pad, true, false),
		("stepped at the grays", &at_grays, stepped, GradientSpread::Pad, false, false),
		("stepped at the grays in reverse", &at_grays, stepped, GradientSpread::Pad, false, true),
		("stepped at the grays, cyclic, and repeating", &at_grays, stepped, GradientSpread::Repeat, true, false),
		("repeating up to white", &at_grays, linear, GradientSpread::Repeat, false, false),
		("repeating down to black in reverse", &at_grays, linear, GradientSpread::Repeat, false, true),
		("clear past its ends", &at_grays, linear, GradientSpread::Clear, false, false),
		("with a hard edge", &hard_edge, linear, GradientSpread::Pad, false, false),
		("reversed", &stops, linear, GradientSpread::Pad, false, true),
		("cyclic and repeating", &wrapping, linear, GradientSpread::Repeat, true, false),
		(
			"smooth, cyclic, and repeating",
			&stops,
			(GradientSpace::Lab, GradientInterpolation::Smooth, GradientHueDirection::Shorter),
			GradientSpread::Repeat,
			true,
			true,
		),
		("reflected with stops past its ends", &outside, linear, GradientSpread::Reflect, false, false),
		(
			"smooth with stops past its ends",
			&outside,
			(GradientSpace::OkLCh, GradientInterpolation::Smooth, GradientHueDirection::Shorter),
			GradientSpread::Pad,
			false,
			false,
		),
		("of a single stop", &single, linear, GradientSpread::Pad, false, false),
		("of no stops", &Vec::new(), linear, GradientSpread::Pad, false, false),
	] {
		runs.push((format!("Gradient Map {name}"), gradient_map(stops, space, interpolation, hue_direction, spread, cyclic, reverse)));
	}
	let mut runs: Vec<(String, Vec<Adjustment>)> = runs.into_iter().map(|(name, adjustment)| (name, vec![adjustment])).collect();

	// Positions past both ends of the ramp, from a lookup table stretching every channel out of gamut, which each spread folds back its own way
	let out_of_gamut = Adjustment::ColorLookup {
		lut_file: graphene_resource::Resource::new(b"LUT_1D_SIZE 2\n-0.6 -0.6 -0.6\n1.7 1.7 1.7\n".to_vec()),
	};
	for (spread, interpolation) in [
		(GradientSpread::Reflect, GradientInterpolation::Linear),
		(GradientSpread::Repeat, GradientInterpolation::Smooth),
		(GradientSpread::Clear, GradientInterpolation::Stepped),
	] {
		let map = gradient_map(&steps, GradientSpace::OkLab, interpolation, GradientHueDirection::Shorter, spread, false, false);
		runs.push((format!("Gradient Map {interpolation:?} with {spread:?} out of gamut"), vec![out_of_gamut.clone(), map]));
	}

	assert_gpu_adjustments_match_the_cpu_kernels(runs);
}

/// An ICC profile of the `class` and color `space` holding one `A2B0` tag.
fn icc_profile(class: &[u8; 4], space: &[u8; 4], tag: &[u8]) -> Vec<u8> {
	let mut bytes = vec![0; 128];
	bytes[8] = 2;
	bytes[12..16].copy_from_slice(class);
	bytes[16..20].copy_from_slice(space);
	bytes[20..24].copy_from_slice(space);
	bytes[36..40].copy_from_slice(b"acsp");

	bytes.extend_from_slice(&1_u32.to_be_bytes());
	bytes.extend_from_slice(b"A2B0");
	bytes.extend_from_slice(&144_u32.to_be_bytes());
	bytes.extend_from_slice(&(tag.len() as u32).to_be_bytes());
	bytes.extend_from_slice(tag);
	bytes
}

/// A value from 0 to 1 as an ICC profile's unsigned 16-bit sample.
fn icc_sample(value: f32) -> [u8; 2] {
	((value.clamp(0., 1.) * 65_535.).round() as u16).to_be_bytes()
}

/// An ICC profile's `s15Fixed16Number`.
fn icc_fixed(value: f32) -> [u8; 4] {
	((value * 65_536.).round() as i32).to_be_bytes()
}

/// An abstract profile with a `lut16Type` table bending CIELAB in its legacy encoding, with output curves after it.
fn icc_abstract_profile() -> Vec<u8> {
	const GRID: usize = 5;
	const OUTPUT_ENTRIES: usize = 16;

	let mut tag = vec![0; 52];
	tag[0..4].copy_from_slice(b"mft2");
	tag[8] = 3;
	tag[9] = 3;
	tag[10] = GRID as u8;
	tag[48..50].copy_from_slice(&2_u16.to_be_bytes());
	tag[50..52].copy_from_slice(&(OUTPUT_ENTRIES as u16).to_be_bytes());

	// Identity input curves, a grid lightening and saturating, then output curves bending gently
	for _ in 0..3 {
		tag.extend([icc_sample(0.), icc_sample(1.)].concat());
	}
	let step = |index: usize| index as f32 / (GRID - 1) as f32;
	for l in 0..GRID {
		for a in 0..GRID {
			for b in 0..GRID {
				let (l, a, b) = (step(l), step(a), step(b));
				for value in [l.powf(0.85), 0.5 + (a - 0.5) * 1.3, 0.5 + (b - 0.5) * 0.8 + 0.05 * l] {
					tag.extend(icc_sample(value));
				}
			}
		}
	}
	for _ in 0..3 {
		tag.extend((0..OUTPUT_ENTRIES).flat_map(|index| icc_sample((index as f32 / (OUTPUT_ENTRIES - 1) as f32).powf(0.9))));
	}

	icc_profile(b"abst", b"Lab ", &tag)
}

/// An abstract profile with a `lutAToBType` transform bending CIELAB in its standard encoding without a grid, by parametric curves of functions 0, 1, and 2
/// around a matrix, among them one with an `a` of 0 and one raising to a power of 0.
fn icc_standard_lab_profile() -> Vec<u8> {
	let parametric = |function: u16, parameters: &[f32]| {
		let mut bytes = b"para\0\0\0\0".to_vec();
		bytes.extend_from_slice(&function.to_be_bytes());
		bytes.extend_from_slice(&[0, 0]);
		bytes.extend(parameters.iter().flat_map(|&parameter| icc_fixed(parameter)));
		bytes
	};

	let mut tag = b"mAB \0\0\0\0\x03\x03\0\0".to_vec();
	tag.extend_from_slice(&[0; 20]);
	let place = |tag: &mut Vec<u8>, elements: Vec<Vec<u8>>| {
		let offset = tag.len() as u32;
		for element in elements {
			tag.extend(element);
			tag.resize(tag.len().next_multiple_of(4), 0);
		}
		offset
	};

	let b_curves = vec![parametric(1, &[1.2, 0.95, 0.02]), parametric(2, &[0.9, 1.1, -0.05, 0.03]), parametric(2, &[1.4, 0., 0.5, 0.1])];
	let b_offset = place(&mut tag, b_curves);
	let matrix = [1., 0.05, 0., 0., 0.9, 0.1, 0., 0., 1., 0., 0.02, -0.03];
	let matrix_offset = place(&mut tag, vec![matrix.iter().flat_map(|&value| icc_fixed(value)).collect()]);
	let m_offset = place(&mut tag, vec![parametric(0, &[0.8]), parametric(0, &[1.]), parametric(0, &[0.])]);

	for (index, offset) in [b_offset, matrix_offset, m_offset].into_iter().enumerate() {
		tag[12 + 4 * index..16 + 4 * index].copy_from_slice(&offset.to_be_bytes());
	}
	icc_profile(b"abst", b"Lab ", &tag)
}

/// A device link profile with a `lutAToBType` transform running sampled curves, a grid of `grid_size` points along each axis, power curves,
/// a matrix with offsets, then parametric curves.
fn icc_device_link_profile(grid_size: u8) -> Vec<u8> {
	let sampled = |samples: &[f32]| {
		let mut bytes = b"curv\0\0\0\0".to_vec();
		bytes.extend_from_slice(&(samples.len() as u32).to_be_bytes());
		bytes.extend(samples.iter().flat_map(|&sample| icc_sample(sample)));
		bytes
	};
	let power = |gamma: f32| {
		let mut bytes = b"curv\0\0\0\0".to_vec();
		bytes.extend_from_slice(&1_u32.to_be_bytes());
		bytes.extend_from_slice(&((gamma * 256.).round() as u16).to_be_bytes());
		bytes
	};
	let parametric = |function: u16, parameters: &[f32]| {
		let mut bytes = b"para\0\0\0\0".to_vec();
		bytes.extend_from_slice(&function.to_be_bytes());
		bytes.extend_from_slice(&[0, 0]);
		bytes.extend(parameters.iter().flat_map(|&parameter| icc_fixed(parameter)));
		bytes
	};

	// Each element starts on a four-byte boundary, located by its offset within the tag
	let mut tag = b"mAB \0\0\0\0\x03\x03\0\0".to_vec();
	tag.extend_from_slice(&[0; 20]);
	let place = |tag: &mut Vec<u8>, elements: Vec<Vec<u8>>| {
		let offset = tag.len() as u32;
		for element in elements {
			tag.extend(element);
			tag.resize(tag.len().next_multiple_of(4), 0);
		}
		offset
	};

	let b_curves = vec![
		parametric(3, &[2.4, 1. / 1.055, 0.055 / 1.055, 1. / 12.92, 0.04045]),
		parametric(4, &[1.8, 0.9, 0.05, 0.5, 0.1, 0.02, 0.01]),
		parametric(0, &[0.7]),
	];
	let b_offset = place(&mut tag, b_curves);
	let matrix = [0.9, 0.1, 0., -0.05, 1.05, 0., 0.1, -0.1, 1., 0.02, 0., -0.01];
	let matrix_offset = place(&mut tag, vec![matrix.iter().flat_map(|&value| icc_fixed(value)).collect()]);
	let m_offset = place(&mut tag, vec![power(1.8), power(1.), power(0.6)]);

	let mut grid = vec![grid_size; 3];
	grid.resize(16, 0);
	grid.extend_from_slice(&[2, 0, 0, 0]);
	for r in 0..grid_size {
		for g in 0..grid_size {
			for b in 0..grid_size {
				let [r, g, b] = [r, g, b].map(|index| index as f32 / (grid_size - 1) as f32);
				for value in [r * 0.8 + g * 0.2, g * g, (b + 0.25 * r * g).min(1.)] {
					grid.extend(icc_sample(value));
				}
			}
		}
	}
	let grid_offset = place(&mut tag, vec![grid]);
	let a_offset = place(&mut tag, vec![sampled(&[0., 0.3, 0.55, 0.8, 1.]), sampled(&[0., 0.1, 0.4, 1.]), sampled(&[0.2, 0.9])]);

	for (index, offset) in [b_offset, matrix_offset, m_offset, grid_offset, a_offset].into_iter().enumerate() {
		tag[12 + 4 * index..16 + 4 * index].copy_from_slice(&offset.to_be_bytes());
	}
	icc_profile(b"link", b"RGB ", &tag)
}

#[test]
fn gpu_color_lookups_match_the_cpu_kernels() {
	use graphic_types::Adjustment;

	let grid = |size: usize| (0..size * size * size).map(move |index| [index % size, index / size % size, index / (size * size)].map(|step| step as f32 / (size - 1) as f32));

	// Three-dimensional CUBE files with red varying fastest, the largest spanning several rows of the texture holding it
	let cube_of_size = |size: usize| {
		let mut cube = format!("TITLE \"Bent\"\nLUT_3D_SIZE {size}\n");
		for [r, g, b] in grid(size) {
			cube += &format!("{} {} {}\n", r.powf(0.8) * 0.9 + 0.1 * g, g * g * 0.7 + 0.2 * b, (b + 0.3 * r * g).min(1.));
		}
		cube
	};
	let cube = cube_of_size(9);

	// A one-dimensional CUBE file over a domain other than 0 to 1
	let mut one_dimensional = String::from("LUT_1D_SIZE 17\nDOMAIN_MIN 0 0.1 0\nDOMAIN_MAX 1 0.9 2\n");
	for index in 0..17 {
		let value = index as f32 / 16.;
		one_dimensional += &format!("{} {} {}\n", value.powf(1.6), value.sqrt(), 1. - value);
	}

	// A 3DL file with blue varying fastest, and an input line uneven enough to shape the values before its 12-bit table
	let mut three_dl = (0..17).map(|index| ((index as f32 / 16.).powf(1.5) * 1023.).round().to_string()).collect::<Vec<_>>().join(" ") + "\nMesh 3 12\n";
	for [b, g, r] in grid(9) {
		let [r, g, b] = [1. - r * 0.6, (g + b) / 2., b * b].map(|value: f32| (value * 4095.).round());
		three_dl += &format!("{r} {g} {b}\n");
	}

	// A CSP file with piecewise input curves, one of them an identity of no points
	let mut csp = String::from("CSPLUTV100\n3D\n\n5\n0 0.2 0.5 0.8 1\n0 0.3 0.6 0.85 1\n1\n3\n0 0.5 1\n0 0.4 1\n5 5 5\n");
	for [r, g, b] in grid(5) {
		csp += &format!("{} {} {}\n", 1. - g, (r + b) / 2., r * g);
	}

	let lookup = |file: Vec<u8>| {
		let lut_file = graphene_resource::Resource::new(file);
		assert!(raster_kernels::color_lookup_table::parse_cached(&lut_file).is_ok(), "A test's table isn't readable");
		Adjustment::ColorLookup { lut_file }
	};
	let cube = lookup(cube.into_bytes());
	let device_link = lookup(icc_device_link_profile(3));
	let standard_lab = lookup(icc_standard_lab_profile());

	// Channels outside 0 to 1 coming into the tables, and a negative one raised to an integer power after one
	let out_of_gamut = lookup(b"LUT_1D_SIZE 2\n-0.6 -0.6 -0.6\n1.7 1.7 1.7\n".to_vec());
	let negative_red = lookup(b"LUT_1D_SIZE 2\n-1 0.5 0.5\n-1 0.5 0.5\n".to_vec());

	let mut runs = vec![
		("Color Lookup of a CUBE file", vec![cube.clone()]),
		("Color Lookup of a 33-point CUBE file", vec![lookup(cube_of_size(33).into_bytes())]),
		("Color Lookup of a one-dimensional CUBE file with a domain", vec![lookup(one_dimensional.into_bytes())]),
		("Color Lookup of a 3DL file with a shaper", vec![lookup(three_dl.into_bytes())]),
		("Color Lookup of a CSP file", vec![lookup(csp.into_bytes())]),
		("Color Lookup of an abstract ICC profile", vec![lookup(icc_abstract_profile())]),
		("Color Lookup of an abstract ICC profile in standard Lab without a grid", vec![standard_lab.clone()]),
		("Color Lookup of a device link ICC profile", vec![device_link.clone()]),
		// A grid of more texels than 16 bits count, which the stages after it lie past
		("Color Lookup of a device link ICC profile with a 65-point grid", vec![lookup(icc_device_link_profile(65))]),
		("Color Lookup of one CUBE file twice over", vec![cube.clone(), cube.clone()]),
		("Color Lookups of a CUBE file out of gamut", vec![out_of_gamut.clone(), cube]),
		("Color Lookups of a device link ICC profile out of gamut", vec![out_of_gamut.clone(), device_link]),
		("Color Lookups of a standard Lab ICC profile out of gamut", vec![out_of_gamut, standard_lab]),
		(
			"Color Lookup to negative red, then Gamma Correction",
			vec![negative_red, Adjustment::GammaCorrection { gamma: 0.5, inverse: false }],
		),
	];

	// A table with steps after it, which share the texture of the steps while the table has its own
	let record = |shadows, midtones, highlights, output_minimums, output_maximums| graphic_types::adjustment::LevelsRecord {
		shadows,
		midtones,
		highlights,
		output_minimums,
		output_maximums,
	};
	let identity = record(0., 1., 100., 0., 100.);
	let levels = Adjustment::Levels(Box::new(graphic_types::adjustment::LevelsSettings {
		composite: record(8., 1.3, 93., 2., 97.),
		red: identity,
		green: identity,
		blue: identity,
		alpha: identity,
	}));
	let curves = Adjustment::Curves(Box::new(graphic_types::adjustment::CurvesSettings {
		composite: core_types::transfer_curve::TransferCurve::new(vec![glam::DVec2::ZERO, glam::DVec2::new(0.4, 0.3), glam::DVec2::ONE]),
		..Default::default()
	}));
	runs.push(("Color Lookup of a 17-point CUBE file, Levels, then Curves", vec![lookup(cube_of_size(17).into_bytes()), levels, curves]));

	assert_gpu_adjustments_match_the_cpu_kernels(runs);
}

#[test]
fn gpu_runs_of_several_adjustments_match_the_cpu_kernels() {
	use core_types::Color;
	use core_types::blending::BlendMode;
	use core_types::list::Item;
	use core_types::transfer_curve::TransferCurve;
	use glam::DVec2;
	use graphic_types::adjustment::*;
	use graphic_types::vector_types::Gradient;

	let record = |shadows, midtones, highlights| LevelsRecord {
		shadows,
		midtones,
		highlights,
		output_minimums: 0.,
		output_maximums: 100.,
	};
	let identity = record(0., 1., 100.);
	let levels = Adjustment::Levels(Box::new(LevelsSettings {
		composite: record(10., 1.4, 90.),
		red: identity,
		green: record(5., 0.8, 100.),
		blue: identity,
		alpha: LevelsRecord { output_maximums: 60., ..identity },
	}));
	let curves = Adjustment::Curves(Box::new(CurvesSettings {
		composite: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(0.3, 0.45), DVec2::ONE]),
		..Default::default()
	}));
	let gradient_map = Adjustment::GradientMap {
		gradient: Item::new_from_element(Gradient::from(vec![Color::from_rgbaf32_unchecked(0.1, 0., 0.4, 1.), Color::from_rgbaf32_unchecked(1., 0.8, 0.2, 1.)])),
		reverse: false,
	};
	let overlay = Adjustment::ColorOverlay {
		color: Color::from_gamma_srgb_channels(0.2, 0.5, 0.9, 1.),
		blend_mode: BlendMode::SoftLight,
		opacity: 60.,
	};
	let exposure = Adjustment::Exposure {
		exposure: 0.5,
		offset: 0.,
		gamma_correction: 1.2,
	};
	let vibrance = Adjustment::Vibrance { vibrance: 60., saturation: 20. };
	let posterize = Adjustment::Posterize { levels: 5 };
	let channel_mixer = Adjustment::ChannelMixer(Box::new(ChannelMixerSettings {
		monochrome: false,
		monochrome_mix: Default::default(),
		red: ChannelMixerRow {
			red: 70.,
			green: 30.,
			..Default::default()
		},
		green: ChannelMixerRow { green: 100., ..Default::default() },
		blue: ChannelMixerRow {
			blue: 80.,
			constant: 10.,
			..Default::default()
		},
	}));
	let threshold = Adjustment::Threshold {
		min_luminance: 37.3,
		max_luminance: 88.1,
	};
	let unreadable_lookup = Adjustment::ColorLookup {
		lut_file: graphene_resource::Resource::new(b"not a table".to_vec()),
	};

	// White passes through Color Balance as white, which a Threshold up to 100% keeps and a Color Burn of black leaves alone
	let color_balance = Adjustment::ColorBalance(ColorBalanceSettings {
		midtones: ColorBalanceShifts {
			cyan_red: 10.,
			magenta_green: -20.,
			yellow_blue: 5.,
		},
		preserve_luminosity: true,
		..Default::default()
	});
	let threshold_to_white = Adjustment::Threshold {
		min_luminance: 50.,
		max_luminance: 100.,
	};
	let color_burn = Adjustment::ColorOverlay {
		color: Color::BLACK,
		blend_mode: BlendMode::ColorBurn,
		opacity: 100.,
	};

	// An alpha curve emptying the faintest pixels, which nothing after it fills back in
	let emptying_curves = Adjustment::Curves(Box::new(CurvesSettings {
		alpha: TransferCurve::new(vec![DVec2::new(0.4, 0.), DVec2::ONE]),
		..Default::default()
	}));

	// A Color Lookup leaving colors as they are, in a pass of its own
	let identity_lookup = Adjustment::ColorLookup {
		lut_file: graphene_resource::Resource::new(b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n".to_vec()),
	};

	let runs = vec![
		(
			"Levels, Curves, Color Overlay, then Gradient Map",
			vec![levels.clone(), curves.clone(), overlay.clone(), gradient_map.clone()],
		),
		("Exposure, Vibrance, Invert, then Posterize", vec![exposure.clone(), vibrance.clone(), Adjustment::Invert, posterize]),
		(
			"Levels fading alpha, Make Opaque, Channel Mixer, then Threshold",
			vec![levels.clone(), Adjustment::MakeOpaque, channel_mixer.clone(), threshold],
		),
		("Vibrance, an unreadable Color Lookup, then Curves", vec![vibrance.clone(), unreadable_lookup.clone(), curves.clone()]),
		("an unreadable Color Lookup alone", vec![unreadable_lookup]),
		("Gradient Map, Color Overlay, then Exposure", vec![gradient_map.clone(), overlay.clone(), exposure.clone()]),
		("Color Balance, then Threshold up to white", vec![color_balance.clone(), threshold_to_white.clone()]),
		(
			"Color Balance at its defaults, then Threshold up to white",
			vec![Adjustment::ColorBalance(Default::default()), threshold_to_white],
		),
		("Color Balance, then a Color Burn of black", vec![color_balance, color_burn]),
		(
			"Curves emptying faint pixels, then Make Opaque",
			vec![emptying_curves.clone(), Adjustment::MakeOpaque, Adjustment::Invert],
		),
		// The pass emptying a pixel ends before the Color Lookup's own pass, and the one after it with Make Opaque still leaves the pixel empty
		(
			"Curves emptying faint pixels, then a Color Lookup and Make Opaque",
			vec![emptying_curves, identity_lookup, Adjustment::MakeOpaque],
		),
	];
	assert_gpu_adjustments_stray_at_most(0.3, runs, &[1., 0.5]);

	// Passes on either side of each Gradient Map, each reading the result of the one before unrounded
	let chained = vec![gradient_map.clone(), curves, channel_mixer, gradient_map, vibrance];
	assert_gpu_adjustments_stray_at_most(0.3, vec![("Gradient Map, Curves, Channel Mixer, Gradient Map, then Vibrance", chained)], &[1., 0.5]);
}

#[test]
fn passes_of_a_split_run_hand_on_their_results_unrounded() {
	use core_types::transfer_curve::TransferCurve;
	use glam::DVec2;
	use graphic_types::adjustment::*;

	// A Color Lookup leaving colors as they are, in a pass of its own that splits the run around it
	let split = Adjustment::ColorLookup {
		lut_file: graphene_resource::Resource::new(b"LUT_1D_SIZE 2\n0 0 0\n1 1 1\n".to_vec()),
	};
	let record = |shadows, highlights, output_maximums| LevelsRecord {
		shadows,
		midtones: 1.,
		highlights,
		output_minimums: 0.,
		output_maximums,
	};
	let identity = record(0., 100., 100.);
	let levels = |composite, alpha| {
		Adjustment::Levels(Box::new(LevelsSettings {
			composite,
			red: identity,
			green: identity,
			blue: identity,
			alpha,
		}))
	};

	// A slight bend leaving tones between those half floats hold, then a tenfold stretch of the middle tones that would show their rounding ten times over
	let bend = Adjustment::GammaCorrection { gamma: 1.1, inverse: false };
	let steep = vec![bend, split.clone(), levels(record(45., 55., 100.), identity)];

	// A gray just over a Threshold's cutoff, where rounding it to a half float either way (as GPUs differ in) would land under it
	let just_over_cutoff = Adjustment::Curves(Box::new(CurvesSettings {
		composite: TransferCurve::new(vec![DVec2::new(0., 0.5002), DVec2::new(1., 0.5002)]),
		..Default::default()
	}));
	let threshold = Adjustment::Threshold {
		min_luminance: 50.01,
		max_luminance: 100.,
	};
	let cutoff = vec![just_over_cutoff, split.clone(), threshold];

	// An alpha too faint for a half float to hold, which Make Opaque then lifts
	let faint = levels(identity, record(0., 100., 2e-6));
	let lifted = vec![faint, split, Adjustment::MakeOpaque];

	// Grays a 4096th apart, hundreds of which land within the stretch
	let grays: Vec<core_types::Color> = (0..=4096)
		.map(|level| level as f32 / 4096.)
		.map(|gray| core_types::Color::from_gamma_srgb_channels(gray, gray, gray, 1.))
		.collect();

	let mut strays = Vec::new();
	for (name, run) in [("a steep Levels", steep), ("a Threshold", cutoff), ("Make Opaque", lifted)] {
		// Each run takes three passes, the last of them holding the step that would reveal any rounding
		let references: Vec<&Adjustment> = run.iter().collect();
		let steps = raster_kernels::adjustment_steps::AdjustmentSteps::encode(&references);
		assert_eq!(crate::vello_adjustments::pass_steps(steps.kinds()), [0..1, 1..2, 2..3], "{name}");

		let Some((levels, worst_pixel)) = gpu_adjustment_deviation(&run, &grays, 1.) else { return };
		println!("{name} after two passes: {levels:.3} levels");
		if levels > 0.3 {
			strays.push(format!("{name} after two passes strays from the CPU's kernels by {levels} levels, adjusting {worst_pixel}"));
		}
	}
	assert!(strays.is_empty(), "{}", strays.join("\n"));
}

#[test]
fn gpu_adjustments_of_8_bit_content_match_the_cpu_kernels_at_the_edges_of_their_steps() {
	use graphic_types::adjustment::*;
	use graphic_types::{EffectOperation, Effects};

	// Every 8-bit gray as one opaque pixel, rendered as exactly that level
	let size = UVec2::splat(16);
	let square = Rect::new(0., 0., 16., 16.);
	let draw_grays = |scene: &mut VelloSceneBuilder| {
		for level in 0..=255_u8 {
			let (x, y) = (f64::from(level % 16), f64::from(level / 16));
			scene.fill(Fill::NonZero, Affine::IDENTITY, Color::from_rgba8(level, level, level, 255), None, &Rect::new(x, y, x + 1., y + 1.));
		}
	};

	// Grays landing exactly on the edges between posterized levels, on the pivot of a full contrast, and at a Levels input bound
	let record = |shadows, highlights| LevelsRecord {
		shadows,
		midtones: 1.,
		highlights,
		output_minimums: 0.,
		output_maximums: 100.,
	};
	let levels = |shadows, highlights| {
		Adjustment::Levels(Box::new(LevelsSettings {
			composite: record(shadows, highlights),
			red: record(0., 100.),
			green: record(0., 100.),
			blue: record(0., 100.),
			alpha: record(0., 100.),
		}))
	};
	let mut cases: Vec<(String, Adjustment)> = [2, 3, 5, 7, 255]
		.into_iter()
		.map(|levels| (format!("Posterize to {levels} levels"), Adjustment::Posterize { levels }))
		.collect();
	for pivot in [0., 64., 100., 127., 128., 191., 255.] {
		let contrast = Adjustment::BrightnessContrast {
			brightness: 0.,
			contrast: 100.,
			use_classic: true,
			classic_pivot: pivot,
		};
		cases.push((format!("Brightness/Contrast classic at full contrast about {pivot}"), contrast));
	}
	for (shadows, highlights) in [(40., 100.), (41., 100.), (0., 40.), (0., 41.)] {
		cases.push((format!("Levels from {shadows}% to {highlights}%"), levels(shadows, highlights)));
	}

	// Thresholds bounded exactly at a gray, which lies within them
	for level in [1_u8, 64, 127, 128, 200, 254, 255] {
		let bound = f64::from(level) / 255. * 100.;
		let threshold = |min_luminance, max_luminance| Adjustment::Threshold { min_luminance, max_luminance };
		cases.push((format!("Threshold from gray {level}"), threshold(bound, 100.)));
		cases.push((format!("Threshold up to gray {level}"), threshold(0., bound)));
	}

	let mut strays = Vec::new();
	for (name, adjustment) in cases {
		let effects = Effects::from_operations([EffectOperation::Adjustment(adjustment.clone())]);
		let Some(pixels) = render(size, |scene| scene.draw_effects(&effects, 1., Some(square), DAffine2::IDENTITY, &mut |scene| draw_grays(scene))) else {
			return;
		};

		let mut grays: Vec<core_types::Color> = (0..=255)
			.map(|level| core_types::Color::from_gamma_srgb_channels(level as f32 / 255., level as f32 / 255., level as f32 / 255., 1.))
			.collect();
		raster_kernels::adjustments::adjust_colors(&adjustment, &mut grays);

		let mut worst = (0_f32, 0);
		for (level, gray) in grays.iter().enumerate() {
			let gpu = pixel(&pixels, size, level as u32 % 16, level as u32 / 16)[0] as f32;
			let deviation = (gpu - gray.to_gamma_srgb_channels()[0] * 255.).abs();
			if deviation > worst.0 {
				worst = (deviation, level);
			}
		}
		println!("{name}: {:.3} levels", worst.0);

		// The rendered level is rounded, after passing through a half float
		if worst.0 > 0.7 {
			strays.push(format!("{name} strays from the CPU's kernel by {} levels at gray {}", worst.0, worst.1));
		}
	}
	assert!(strays.is_empty(), "{}", strays.join("\n"));
}

#[test]
fn steep_adjustments_of_smooth_gradients_stay_smooth() {
	use graphic_types::adjustment::*;
	use graphic_types::{EffectOperation, Effects};

	// Levels stretching the middle fifth of the tones across all of them brings out any rounding of the content before it
	let record = |shadows, highlights| LevelsRecord {
		shadows,
		midtones: 1.,
		highlights,
		output_minimums: 0.,
		output_maximums: 100.,
	};
	let identity = record(0., 100.);
	let levels = Adjustment::Levels(Box::new(LevelsSettings {
		composite: record(40., 60.),
		red: identity,
		green: identity,
		blue: identity,
		alpha: identity,
	}));
	let effects = Effects::from_operations([EffectOperation::Adjustment(levels)]);

	let width = 1024_u32;
	let size = UVec2::new(width, 8);
	let bounds = Rect::new(0., 0., f64::from(width), 8.);
	let gradient = peniko::Gradient::new_linear((0., 0.), (f64::from(width), 0.)).with_stops([Color::BLACK, Color::WHITE]);

	// A layer just short of opaque is drawn in an intermediate texture of its own
	for in_layer in [false, true] {
		let Some(pixels) = render(size, |scene| {
			scene.draw_effects(&effects, 1., Some(bounds), DAffine2::IDENTITY, &mut |scene| {
				if in_layer {
					scene.push_layer(Fill::NonZero, peniko::Mix::Normal, 0.999, Affine::IDENTITY, &bounds);
				}
				scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &bounds);
				if in_layer {
					scene.pop_layer();
				}
			});
		}) else {
			return;
		};

		// The stretched tones climb about 1.25 levels a pixel, where content rounded to 8 bits would climb 5 levels every 4 pixels
		let ramp: Vec<i32> = (0..width).map(|x| i32::from(pixel(&pixels, size, x, 4)[0])).filter(|&value| value > 5 && value < 250).collect();
		let largest_step = ramp.windows(2).map(|pair| (pair[1] - pair[0]).abs()).max().unwrap_or(0);
		let mut distinct = ramp.clone();
		distinct.dedup();
		assert!(
			largest_step <= 2 && distinct.len() * 10 >= ramp.len() * 9,
			"the gradient (in a layer: {in_layer}) bands, climbing by up to {largest_step} levels through {} distinct levels across {} pixels",
			distinct.len(),
			ramp.len()
		);
	}
}

#[test]
fn gpu_adjustments_that_keep_white_do_not_carry_it_past_white() {
	use graphic_types::adjustment::*;
	use half::f16;

	let identity = LevelsRecord {
		shadows: 0.,
		midtones: 1.,
		highlights: 100.,
		output_minimums: 0.,
		output_maximums: 100.,
	};
	let levels = |midtones| {
		Adjustment::Levels(Box::new(LevelsSettings {
			composite: LevelsRecord { midtones, ..identity },
			red: identity,
			green: identity,
			blue: identity,
			alpha: identity,
		}))
	};
	let color_balance = |cyan_red| {
		Adjustment::ColorBalance(ColorBalanceSettings {
			midtones: ColorBalanceShifts { cyan_red, ..Default::default() },
			preserve_luminosity: true,
			..Default::default()
		})
	};
	let cases = [
		("Levels", levels(1.)),
		("Levels with bent midtones", levels(1.2)),
		("Color Balance", color_balance(0.)),
		("Color Balance with shifted midtones", color_balance(10.)),
		("Curves", Adjustment::Curves(Default::default())),
		("Posterize", Adjustment::Posterize { levels: 4 }),
		(
			"Exposure",
			Adjustment::Exposure {
				exposure: 0.,
				offset: 0.,
				gamma_correction: 1.,
			},
		),
		(
			"Brightness/Contrast",
			Adjustment::BrightnessContrast {
				brightness: 0.,
				contrast: 0.,
				use_classic: false,
				classic_pivot: 127.,
			},
		),
		("Hue/Saturation", Adjustment::HueSaturation(Default::default())),
		("Selective Color", Adjustment::SelectiveColor(Default::default())),
		("Gamma Correction", Adjustment::GammaCorrection { gamma: 2.2, inverse: false }),
		("Vibrance", Adjustment::Vibrance { vibrance: 20., saturation: 0. }),
		(
			"Photo Filter",
			Adjustment::PhotoFilter {
				color: core_types::Color::WHITE,
				density: 25.,
				preserve_luminosity: false,
			},
		),
	];

	// White, opaque and translucent, as premultiplied half floats
	let whites = [[1.; 4], [0.5; 4]].map(|texel: [f32; 4]| texel.map(f16::from_f32));

	// A half float rounds a channel left a hair past 1 back to 1, hiding it, but its inverse falls below zero where the sign shows
	for (name, adjustment) in cases {
		let Some(inverted) = gpu_adjusted_texels(&[adjustment, Adjustment::Invert], &whites, 1.) else {
			return;
		};

		for [red, green, blue, _] in inverted {
			for channel in [red, green, blue] {
				assert!(channel.is_sign_positive() && channel < 1e-3, "{name} leaves white at {channel} once inverted");
			}
		}
	}
}

#[test]
fn adjustments_leave_the_empty_space_around_content_empty() {
	use core_types::transfer_curve::TransferCurve;
	use glam::DVec2;
	use graphic_types::adjustment::*;
	use graphic_types::{EffectOperation, Effects};

	let size = UVec2::splat(128);
	let bounds = Rect::new(40., 40., 88., 88.);
	let identity = LevelsRecord {
		shadows: 0.,
		midtones: 1.,
		highlights: 100.,
		output_minimums: 0.,
		output_maximums: 100.,
	};

	// Adjustments raising alpha, the last after a curve that empties everything first
	let raised_alpha = Adjustment::Levels(Box::new(LevelsSettings {
		composite: identity,
		red: identity,
		green: identity,
		blue: identity,
		alpha: LevelsRecord { output_minimums: 50., ..identity },
	}));
	let emptied = Adjustment::Curves(Box::new(CurvesSettings {
		alpha: TransferCurve::new(vec![DVec2::ZERO, DVec2::new(1., 0.)]),
		..Default::default()
	}));
	let cases = [
		("Make Opaque", vec![Adjustment::MakeOpaque], 255),
		("Levels with a raised alpha output", vec![raised_alpha], 255),
		("Curves emptying alpha, then Make Opaque", vec![emptied, Adjustment::MakeOpaque], 0),
	];

	for (name, run, alpha_within) in cases {
		let effects = Effects::from_operations(run.into_iter().map(EffectOperation::Adjustment));
		let Some(pixels) = render(size, |scene| {
			scene.draw_effects(&effects, 1., Some(bounds), DAffine2::IDENTITY, &mut |scene| {
				scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((64., 64.), 20.))
			});
		}) else {
			return;
		};

		// The content keeps its own alpha as adjusted, and the corner of its bounds outside it (like everything past them) holds nothing
		assert_eq!(pixel(&pixels, size, 64, 64)[3], alpha_within, "{name} within the content");
		assert_eq!(pixel(&pixels, size, 42, 42), [0; 4], "{name} within the content's bounds");
		assert_eq!(pixel(&pixels, size, 4, 4), [0; 4], "{name} past the content's bounds");
	}
}

#[test]
fn adjustments_on_image_textures_and_through_effects_groups_match_the_cpu_kernels() {
	use core_types::Color as GraphicColor;
	use core_types::list::Item;
	use graphic_types::adjustment::*;
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::vector_types::Gradient;
	use graphic_types::{EffectOperation, Effects};

	// Opaque colors spread across the RGB cube, one per pixel, on whole 8-bit levels as the content's 8-bit render holds them
	let size = UVec2::new(64, 64);
	let channel = |value: u32| ((value % 64) * 4) as f32 / 255.;
	let colors: Vec<GraphicColor> = (0..size.x * size.y)
		.map(|index| GraphicColor::from_gamma_srgb_channels(channel(index % 64), channel(index / 64), channel(index * 7 + index / 64 * 13), 1.))
		.collect();
	let raster = Raster::new_cpu(Image {
		width: size.x,
		height: size.y,
		data: colors.clone(),
	});

	let record = |shadows, midtones, highlights| LevelsRecord {
		shadows,
		midtones,
		highlights,
		output_minimums: 0.,
		output_maximums: 100.,
	};
	let identity = record(0., 1., 100.);
	let levels = Adjustment::Levels(Box::new(LevelsSettings {
		composite: record(10., 1.4, 90.),
		red: identity,
		green: record(5., 0.8, 100.),
		blue: identity,
		alpha: identity,
	}));
	let hue_saturation = Adjustment::HueSaturation(Box::new(HueSaturationSettings {
		master: HueSaturationSliders {
			hue: 25.,
			saturation: 30.,
			lightness: 5.,
		},
		..Default::default()
	}));
	let gradient_map = Adjustment::GradientMap {
		gradient: Item::new_from_element(Gradient::from(vec![
			GraphicColor::from_rgbaf32_unchecked(0.1, 0., 0.4, 1.),
			GraphicColor::from_rgbaf32_unchecked(1., 0.8, 0.2, 1.),
		])),
		reverse: false,
	};
	let exposure = Adjustment::Exposure {
		exposure: 0.5,
		offset: 0.,
		gamma_correction: 1.2,
	};
	let vibrance = |vibrance, saturation| Adjustment::Vibrance { vibrance, saturation };

	// Runs of one kind and of several, each in one pass however many kinds it mixes, but for the last, which its Gradient Map splits into three
	let runs = [
		vec![levels.clone(), exposure, Adjustment::Invert],
		vec![hue_saturation, Adjustment::Desaturate { method: DesaturateMethod::LumaRec601 }],
		vec![gradient_map.clone()],
		vec![vibrance(60., 20.)],
		vec![vibrance(40., 0.), Adjustment::Invert],
		vec![Adjustment::Invert, vibrance(-30., 10.), Adjustment::Invert],
		vec![levels, gradient_map, vibrance(30., 10.)],
	];

	for run in runs {
		let mut expected = colors.clone();
		run.iter().for_each(|adjustment| raster_kernels::adjustments::adjust_colors(adjustment, &mut expected));

		// Carried out on the rendered content by an effects group, and on the image's own texture as it's drawn
		let effects = Effects::from_operations(run.iter().cloned().map(EffectOperation::Adjustment));
		let through_effects_group = |scene: &mut VelloSceneBuilder| {
			let bounds = Rect::new(0., 0., size.x as f64, size.y as f64);
			scene.draw_effects(&effects, 1., Some(bounds), DAffine2::IDENTITY, &mut |scene| {
				scene.draw_raster(raster.clone(), Vec::new(), peniko::ImageSampler::new(), Affine::IDENTITY)
			});
		};
		let on_image_texture = |scene: &mut VelloSceneBuilder| scene.draw_raster(raster.clone(), run.clone(), peniko::ImageSampler::new(), Affine::IDENTITY);
		let paths = [
			("through an effects group", &through_effects_group as &dyn Fn(&mut VelloSceneBuilder)),
			("on the image's texture", &on_image_texture),
		];

		// Only the 8-bit rounding of the content and the result separates them
		for (path, draw) in paths {
			let Some(gpu) = render(size, draw) else { return };

			let mut worst = 0;
			for (index, color) in expected.iter().enumerate() {
				let cpu = color.to_gamma_srgb_channels().map(|channel| (channel * 255.).round() as i32);
				for channel in 0..3 {
					worst = worst.max((cpu[channel] - gpu[index][channel] as i32).unsigned_abs());
				}
			}
			let names = run.iter().map(|adjustment| adjustment.kind().to_string()).collect::<Vec<_>>().join(" + ");
			assert!(worst <= 1, "{names} {path} differs from the CPU kernels by up to {worst} levels");
		}
	}
}

#[test]
fn minified_images_average_their_pixels() {
	use raster_types::{Image, Raster};

	// Stripes half black and half white in every eight columns, drawn at an eighth of their size, where sampling only the two
	// columns nearest each pixel's center would find black
	let stripes = Image {
		width: 64,
		height: 64,
		data: (0..64 * 64)
			.map(|index| if (2..6).contains(&(index % 8)) { core_types::Color::BLACK } else { core_types::Color::WHITE })
			.collect(),
	};
	let stripes = Raster::new_cpu(stripes);
	let draw = |scene: &mut VelloSceneBuilder| scene.draw_raster(stripes.clone(), Vec::new(), peniko::ImageSampler::new(), Affine::scale(1. / 8.));

	let size = UVec2::new(8, 8);
	let Some(pixels) = render(size, draw) else { return };
	for (index, pixel) in pixels.iter().enumerate() {
		assert!(
			(115..=140).contains(&pixel[0]) && pixel[1] == pixel[0] && pixel[2] == pixel[0] && pixel[3] == 255,
			"pixel {index} is {pixel:?} rather than the gray of its average"
		);
	}
}

#[test]
fn minified_adjusted_images_sample_their_adjusted_mipmaps() {
	use graphic_types::Adjustment;
	use graphic_types::raster_types::{Image, Raster};

	// White inverted to black, drawn at an eighth of its size where only the smaller levels are sampled
	let white = Raster::new_cpu(Image::new(64, 64, core_types::Color::WHITE));
	let draw = |scene: &mut VelloSceneBuilder| scene.draw_raster(white.clone(), vec![Adjustment::Invert], peniko::ImageSampler::new(), Affine::scale(1. / 8.));

	let Some(pixels) = render(UVec2::new(8, 8), draw) else { return };
	for (index, pixel) in pixels.iter().enumerate() {
		assert!(pixel[0] <= 1 && pixel[3] == 255, "pixel {index} is {pixel:?} rather than black");
	}
}

#[test]
fn rasters_upload_once_across_scenes_until_modified() {
	use raster_types::{Image, Raster};

	let Some(executor) = executor() else { return };
	let _turn = take_turn();
	let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
	let raster = Raster::new_cpu(Image::new(8, 8, core_types::Color::WHITE));

	// Each render draws its scene anew, as the editor does whenever the graph runs
	let scene_drawing = |raster: &Raster<raster_types::CPU>| {
		let mut scene = VelloSceneBuilder::new(64, 64, Affine::IDENTITY, max_texture_size);
		scene.draw_raster(raster.clone(), Vec::new(), peniko::ImageSampler::new(), Affine::scale(8.));
		scene
	};

	let mut canvas = crate::VelloCanvas::default();
	for _ in 0..3 {
		executor.render_vello_scene_on_canvas(&mut canvas, scene_drawing(&raster)).unwrap();
	}
	assert_eq!(canvas.images.textures.len(), 1, "a clone of the same pixels reuses their texture");

	let mut modified = raster.clone();
	modified.data_mut().data[0] = core_types::Color::BLACK;
	executor.render_vello_scene_on_canvas(&mut canvas, scene_drawing(&modified)).unwrap();
	assert_eq!(canvas.images.textures.len(), 2, "modified pixels get a texture of their own");
}

#[test]
fn textures_of_dropped_rasters_are_freed() {
	use raster_types::{Image, Raster};

	let Some(executor) = executor() else { return };
	let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;

	// Each render draws new pixels and drops the last ones, as dragging a slider on a raster node does
	let mut canvas = crate::VelloCanvas::default();
	for _ in 0..10 {
		let mut scene = VelloSceneBuilder::new(64, 64, Affine::IDENTITY, max_texture_size);
		scene.draw_raster(Raster::new_cpu(Image::new(8, 8, core_types::Color::WHITE)), Vec::new(), peniko::ImageSampler::new(), Affine::scale(8.));
		executor.render_vello_scene_on_canvas(&mut canvas, scene).unwrap();
	}
	assert_eq!(canvas.images.textures.len(), 1, "only the texture of pixels that still exist stays");
}

#[test]
fn svgs_draw_content_with_effects_svg_cannot_express_as_images_the_gpu_renders() {
	use core_types::list::{Item, List};
	use core_types::transform::Footprint;
	use core_types::{ATTR_TRANSFORM, Color};
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::{Adjustment, Effect, EffectOperation, stamp_effect};
	use rendering::{Render, RenderParams, RenderSvgSegmentList, SvgRender};

	let Some(executor) = executor() else { return };
	let _turn = take_turn();

	// An orange square casting a shadow, with Vibrance draining its color, which no SVG filter can do after the shadow
	let shadow = EffectOperation::DropShadow {
		color: Color::BLACK,
		angle: 180.,
		distance: 4.,
		spread: 0.,
		size: 0.,
		contour: Default::default(),
		knockout: true,
	};
	let mut item =
		Item::new_from_element(Raster::new_cpu(Image::new(2, 2, Color::from_gamma_srgb_channels(1., 0.5, 0., 1.)))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(glam::DVec2::splat(10.)));
	stamp_effect(&mut item, Effect::new(&shadow));
	stamp_effect(&mut item, Effect::new(&EffectOperation::Adjustment(Adjustment::Vibrance { vibrance: 0., saturation: -100. })));

	let mut render = SvgRender::new();
	List::new_from_item(item).render_svg(
		&mut render,
		&RenderParams {
			for_export: true,
			..Default::default()
		},
	);
	let footprint = Footprint {
		transform: DAffine2::IDENTITY,
		resolution: UVec2::splat(32),
		..Default::default()
	};
	let rendered = executor.render_deferred_svg_images(&render.deferred_images, footprint);

	// The square comes out gray, with the shadow beside it
	let [(_, texture)] = &rendered[..] else { panic!("expected one image, got {}", rendered.len()) };
	let pixels = executor.read_texture_pixels(texture).unwrap();
	let width = texture.width() as usize;
	let [red, green, blue, alpha] = pixels[(5 * width + 2) * 4..][..4] else { unreachable!() };
	assert!(
		red.abs_diff(green) <= 2 && green.abs_diff(blue) <= 2 && alpha == 255,
		"expected gray, got {:?}",
		[red, green, blue, alpha]
	);
	assert_eq!(pixels[(5 * width + 12) * 4..][..4], [0, 0, 0, 255], "the shadow");

	// Its image replaces the markup standing in for it
	let mut svg = render.svg.to_svg_string();
	futures::executor::block_on(executor.resolve_deferred_svg_images(&mut svg, &mut Vec::new(), rendered));
	assert!(svg.contains("data:image/png;base64,") && !svg.contains("deferred-image"), "{svg}");
}

#[test]
fn adjusted_images_remake_one_copy_as_their_adjustments_change() {
	use graphic_types::Adjustment;
	use graphic_types::raster_types::{Image, Raster};

	let Some(executor) = executor() else { return };
	let _turn = take_turn();
	let new_scene = || VelloSceneBuilder::new(64, 64, Affine::IDENTITY, executor.context().device.limits().max_texture_dimension_2d);
	let raster = Raster::new_cpu(Image::new(8, 8, core_types::Color::WHITE));
	let exposure = |exposure| Adjustment::Exposure {
		exposure,
		offset: 0.,
		gamma_correction: 1.,
	};
	let copies = |canvas: &crate::VelloCanvas| canvas.images.textures.values().map(|image_texture| image_texture.adjusted.len()).sum::<usize>();

	// Dragging a slider adjusts the same pixels differently in every render
	let mut canvas = crate::VelloCanvas::default();
	for amount in [0.1, 0.2, 0.3] {
		let mut scene = new_scene();
		scene.draw_raster(raster.clone(), vec![exposure(amount)], peniko::ImageSampler::new(), Affine::scale(8.));
		executor.render_vello_scene_on_canvas(&mut canvas, scene).unwrap();
	}
	assert_eq!(copies(&canvas), 1, "each new run takes over the copy the last render drew");

	// Two layers sharing the pixels with different adjustments need a copy each
	let mut scene = new_scene();
	scene.draw_raster(raster.clone(), vec![exposure(0.4)], peniko::ImageSampler::new(), Affine::scale(4.));
	scene.draw_raster(raster.clone(), vec![Adjustment::Invert], peniko::ImageSampler::new(), Affine::scale(8.));
	executor.render_vello_scene_on_canvas(&mut canvas, scene).unwrap();
	assert_eq!(copies(&canvas), 2);
}

/// The settings of a noise pattern with features a few units across.
fn noise_settings() -> graphic_types::generator::NoiseSettings {
	use graphic_types::generator::*;

	NoiseSettings {
		seed: 11,
		scale: 6.,
		noise_type: NoiseType::OpenSimplex2,
		domain_warp_type: DomainWarpType::None,
		domain_warp_amplitude: 0.,
		fractal_type: FractalType::FBm,
		fractal_octaves: 3,
		fractal_lacunarity: 2.,
		fractal_gain: 0.5,
		fractal_weighted_strength: 0.,
		fractal_ping_pong_strength: 2.,
		cellular_distance_function: CellularDistanceFunction::Euclidean,
		cellular_return_type: CellularReturnType::Nearest,
		cellular_jitter: 1.,
	}
}

/// A noise pattern with features a few units across.
fn noise_pattern() -> graphic_types::generator::GeneratorPattern {
	graphic_types::generator::GeneratorPattern::Noise(noise_settings())
}

/// Draws `pattern` with `pixels_per_unit` pixels to a unit, from `origin` in its space, comparing 64 by 64 pixels with the generator's own samples.
/// Returns how many pixels differ by more than 2 levels and the most any differs by, or `None` without a GPU.
fn drawing_differences(pattern: &graphic_types::generator::GeneratorPattern, pixels_per_unit: f64, origin: glam::DVec2) -> Option<(usize, u8)> {
	let size = UVec2::new(80, 80);
	let pixels = render(size, |scene| {
		scene.draw_generator(pattern, Affine::translate((8., 8.)) * Affine::scale(pixels_per_unit) * Affine::translate((-origin.x, -origin.y)))
	})?;

	// The pattern fills the plane, past the area compared
	assert_eq!(pixel(&pixels, size, 2, 2)[3], 255);
	assert_eq!(pixel(&pixels, size, 76, 76)[3], 255);

	let (expected, _, _) = raster_kernels::generators::render_pattern(pattern, [origin, origin + 64. / pixels_per_unit], 64, 64)
		.unwrap()
		.to_flat_u8();
	let differences: Vec<u8> = (0..64 * 64)
		.map(|index| {
			let drawn = pixel(&pixels, size, index % 64 + 8, index / 64 + 8);
			let sampled = &expected[(index * 4) as usize..][..4];
			(0..3).map(|channel| drawn[channel].abs_diff(sampled[channel])).max().unwrap_or(0)
		})
		.collect();

	let mismatched = differences.iter().filter(|&&difference| difference > 2).count();
	Some((mismatched, differences.into_iter().max().unwrap_or(0)))
}

#[test]
fn generators_draw_their_pattern_at_the_resolution_in_view() {
	// Each unit of the pattern drawn across two pixels, so its tiles sample the pattern at every pixel's center
	let Some((_, worst)) = drawing_differences(&noise_pattern(), 2., glam::DVec2::ZERO) else { return };
	assert!(worst <= 2, "the drawn pattern differs from the generator's own samples by up to {worst} levels");
}

#[test]
fn every_noise_setting_renders_on_the_gpu_like_on_the_cpu() {
	use core_types::choice_type::ChoiceTypeStatic;
	use graphic_types::generator::*;

	fn every<T: ChoiceTypeStatic + 'static>() -> impl Iterator<Item = T> {
		T::list().iter().flat_map(|section| section.iter().map(|(variant, _)| *variant))
	}

	// Each noise type, fractal type, and domain warp, and each pairing of the cellular noise's distance function and return type
	let base = || NoiseSettings {
		fractal_type: FractalType::None,
		..noise_settings()
	};
	let warps = [FractalType::None, FractalType::DomainWarpProgressive, FractalType::DomainWarpIndependent];
	let variations = (every::<NoiseType>().map(|noise_type| NoiseSettings { noise_type, ..base() }))
		.chain(every::<FractalType>().map(|fractal_type| NoiseSettings {
			fractal_type,
			fractal_weighted_strength: 0.5,
			..base()
		}))
		.chain(every::<DomainWarpType>().flat_map(|domain_warp_type| {
			warps.map(|fractal_type| NoiseSettings {
				domain_warp_type,
				domain_warp_amplitude: 4.,
				fractal_type,
				..base()
			})
		}))
		.chain(every::<CellularDistanceFunction>().flat_map(|cellular_distance_function| {
			every::<CellularReturnType>().map(move |cellular_return_type| NoiseSettings {
				noise_type: NoiseType::Cellular,
				cellular_distance_function,
				cellular_return_type,
				..base()
			})
		}));

	// Another GPU's rounding may put a few pixels on the other side of a cell's edge
	for settings in variations {
		let Some((mismatched, _)) = drawing_differences(&GeneratorPattern::Noise(settings.clone()), 2., glam::DVec2::ZERO) else {
			return;
		};
		assert!(mismatched <= 8, "{mismatched} pixels differ from the CPU's samples with {settings:?}");
	}
}

#[test]
fn every_mandelbrot_setting_renders_on_the_gpu_like_on_the_cpu() {
	use core_types::choice_type::ChoiceTypeStatic;
	use graphic_types::generator::*;

	fn every<T: ChoiceTypeStatic + 'static>() -> impl Iterator<Item = T> {
		T::list().iter().flat_map(|section| section.iter().map(|(variant, _)| *variant))
	}

	let GeneratorPattern::Mandelbrot(defaults) = GeneratorPattern::decode(Generator::Mandelbrot, &|_| None) else {
		unreachable!()
	};

	// A power-of-two scale, so the tiles' pixels line up with the pixels compared at the power-of-two zooms below
	let scale = 1024.;

	// Each fractal, interior, and exterior, and every other setting away from its default
	let base = || MandelbrotSettings { scale, ..defaults.clone() };
	let variations = (every::<MandelbrotFractal>().map(|fractal| MandelbrotSettings { fractal, ..base() }))
		.chain(every::<MandelbrotExterior>().map(|exterior| MandelbrotSettings { exterior, ..base() }))
		.chain(every::<MandelbrotInterior>().map(|interior| MandelbrotSettings { interior, ..base() }))
		.chain([50., 100.].map(|mandelbrot_to_julia| MandelbrotSettings { mandelbrot_to_julia, ..base() }))
		.chain([3., 2.5].map(|power| MandelbrotSettings { power, ..base() }))
		.chain([32, 1000].map(|iterations| MandelbrotSettings { iterations, ..base() }))
		.chain([MandelbrotSettings {
			mandelbrot_to_julia: 100.,
			julia_slice: glam::DVec2::new(0.285, 0.01),
			..base()
		}])
		.chain([0., 50.].map(|mandelbrot_to_julia| MandelbrotSettings {
			mandelbrot_slice: glam::DVec2::new(0.3, -0.2),
			mandelbrot_to_julia,
			..base()
		}))
		.chain([MandelbrotExterior::Tendrils, MandelbrotExterior::Rays].map(|exterior| MandelbrotSettings { exterior, frequency: 2, ..base() }));

	// The plane from -2 to 2 across the pixels compared, where 32-bit rounding, and the CPU and each GPU tile following reference orbits of their own, may tip points
	// on the set's ragged edge into escaping at another iteration
	for settings in variations {
		let Some((mismatched, _)) = drawing_differences(&GeneratorPattern::Mandelbrot(settings.clone()), 64. / (4. * scale), glam::DVec2::splat(-2. * scale)) else {
			return;
		};
		assert!(mismatched <= 120, "{mismatched} pixels differ from the CPU's samples with {settings:?}");
	}

	// Pixels 2^-28 of a unit of the plane across, too fine for 32-bit floats without a reference orbit, on the edge of the set's second largest bulb
	let Some((_, worst)) = drawing_differences(&GeneratorPattern::Mandelbrot(base()), 2_f64.powi(28) / scale, glam::DVec2::new(-1.25 * scale, 0.)) else {
		return;
	};
	assert!(worst <= 2, "deep zooms differ from the CPU's samples by up to {worst} levels");
}

/// Renders the tile of `pattern`, `pixels` across with its top left corner at `corner` of its space and pixels `texel` units across, into straight-alpha RGBA8 pixels, or `None` without a GPU.
fn generator_tile_pixels(pattern: &graphic_types::generator::GeneratorPattern, corner: glam::DVec2, texel: f64, pixels: u32) -> Option<Vec<[u8; 4]>> {
	let executor = executor()?;
	let device = &executor.context().device;
	let _turn = take_turn();

	let texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("generator_tile_test"),
		size: wgpu::Extent3d {
			width: pixels,
			height: pixels,
			depth_or_array_layers: 1,
		},
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: crate::vello_render::TEXTURE_FORMAT,
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
		view_formats: &[],
	});

	let queue = executor.context().queue.lock();
	let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("generator_tile_test") });
	(executor.inner.vello_renderer.generator_passes).render_tile(device, &queue, &mut encoder, &texture, pattern, corner, texel);
	queue.submit([encoder.finish()]);
	drop(queue);

	let bytes = executor.read_texture_pixels(&texture).expect("Failed to read back the tile");
	Some(bytes.as_chunks::<4>().0.to_vec())
}

#[test]
fn deep_mandelbrot_tiles_follow_their_reference_orbits_like_the_cpu() {
	use graphic_types::generator::*;
	use raster_kernels::mandelbrot::MandelbrotSetup;

	let GeneratorPattern::Mandelbrot(defaults) = GeneratorPattern::decode(Generator::Mandelbrot, &|_| None) else {
		unreachable!()
	};

	// Pixels 2^-32 of a unit of the plane across, on the edges of the sets with powers of 2 and 2.5 at 256 iterations, and across the seam of the Tricorn's with a power of 2.5
	// on the negative real axis. Rounding, especially by Direct3D's transcendental functions, may tip a few points on the edges into escaping at another iteration.
	let views = [
		(MandelbrotFractal::ClassicMandelbrot, 2., glam::DVec2::new(0.139_355_853_002_853_52, 0.613_550_669_256_420_2)),
		(MandelbrotFractal::ClassicMandelbrot, 2.5, glam::DVec2::new(0.172_648_565_655_390_94, 0.688_459_272_724_629_5)),
		(MandelbrotFractal::Tricorn, 2.5, glam::DVec2::new(-0.863_244_635_611_779_1, 0.)),
	];
	for (fractal, power, center) in views {
		let settings = MandelbrotSettings {
			fractal,
			power,
			scale: 1.,
			..defaults.clone()
		};
		let (pixels, texel) = (64, 2_f64.powi(-32));
		let corner = center - texel * (pixels / 2) as f64;
		let Some(drawn) = generator_tile_pixels(&GeneratorPattern::Mandelbrot(settings.clone()), corner, texel, pixels) else {
			return;
		};

		// The CPU follows the same reference orbit as the tile
		let setup = MandelbrotSetup::new(&settings);
		let reference = setup.reference_orbit([corner, corner + texel * pixels as f64], texel).filter(|reference| reference.values.len() >= 2);
		assert!(reference.is_some(), "{fractal:?} with a power of {power} has no reference orbit");
		let mismatched = (0..pixels * pixels)
			.filter(|index| {
				let point = corner + (glam::DVec2::new((index % pixels) as f64, (index / pixels) as f64) + 0.5) * texel;
				let sampled = (setup.sample(point, reference.as_ref(), false).clamp(0., 1.) * 255.).round() as u8;
				drawn[*index as usize][0].abs_diff(sampled) > 2
			})
			.count();
		assert!(mismatched <= 128, "{mismatched} pixels differ from the CPU's samples for {fractal:?} with a power of {power}");
	}
}

#[test]
fn generator_tiles_are_reused_across_a_pan() {
	let Some(executor) = executor() else { return };
	let _turn = take_turn();
	let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
	let tiles = |canvas: &crate::VelloCanvas| canvas.images.textures.len();

	// A pattern drawn two pixels to a unit, in tiles of 256 pixels, panned by part of a tile
	let mut canvas = crate::VelloCanvas::default();
	let draw = |offset: f64| {
		let mut scene = VelloSceneBuilder::new(512, 512, Affine::IDENTITY, max_texture_size);
		scene.draw_generator(&noise_pattern(), Affine::translate((offset, 0.)) * Affine::scale(2.));
		scene
	};
	executor.render_vello_scene_on_canvas(&mut canvas, draw(0.)).unwrap();
	let first = tiles(&canvas);
	executor.render_vello_scene_on_canvas(&mut canvas, draw(0.)).unwrap();
	assert_eq!(tiles(&canvas), first, "drawing the same view again renders no tiles");

	executor.render_vello_scene_on_canvas(&mut canvas, draw(-100.)).unwrap();
	let panned = tiles(&canvas) - first;
	assert!(panned < first, "panning renders only the tiles newly in view, not all {first} again (rendered {panned})");
}

#[test]
fn new_generator_tiles_take_over_the_textures_of_freed_ones() {
	use graphic_types::generator::{GeneratorPattern, NoiseSettings};

	let Some(executor) = executor() else { return };
	let _turn = take_turn();
	let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
	let tile_textures = |canvas: &crate::VelloCanvas| -> Vec<wgpu::Texture> {
		(canvas.images.textures.iter())
			.filter(|(key, _)| matches!(key, super::ImageKey::GeneratorTile(_)))
			.map(|(_, image_texture)| image_texture.texture.clone())
			.collect()
	};

	let mut canvas = crate::VelloCanvas::default();
	let draw = |canvas: &mut crate::VelloCanvas, seed: i64| {
		let mut scene = VelloSceneBuilder::new(512, 512, Affine::IDENTITY, max_texture_size);
		scene.draw_generator(&GeneratorPattern::Noise(NoiseSettings { seed, ..noise_settings() }), Affine::scale(2.));
		executor.render_vello_scene_on_canvas(canvas, scene).unwrap();
	};

	// The first pattern's tiles, freed once another is drawn for long enough
	draw(&mut canvas, 1);
	let freed = tile_textures(&canvas);
	for _ in 0..super::UNUSED_IMAGE_RENDER_LIMIT {
		draw(&mut canvas, 2);
	}

	// The textures in use since the first pattern was freed are the second's and the third's, which takes over the first's
	let second = tile_textures(&canvas);
	draw(&mut canvas, 3);
	let third: Vec<_> = tile_textures(&canvas).into_iter().filter(|texture| !second.contains(texture)).collect();
	assert!(
		third.len() == freed.len() && third.iter().all(|texture| freed.contains(texture)),
		"the third pattern's tiles render into the textures of the first's"
	);
}
