//! Renders scenes that stress Vello GPU on the machine's GPU, skipping when none is available.

use crate::WgpuExecutor;
use glam::{DAffine2, UVec2};
use rendering::vello_scene_builder::VelloSceneBuilder;
use vello_gpu::kurbo::{Affine, BezPath, Line, Rect, Stroke};
use vello_gpu::peniko::{self, Color, Fill};

/// The executor every test shares, or `None` without a GPU.
fn executor() -> Option<&'static WgpuExecutor> {
	static EXECUTOR: std::sync::OnceLock<Option<WgpuExecutor>> = std::sync::OnceLock::new();
	EXECUTOR.get_or_init(|| futures::executor::block_on(WgpuExecutor::new())).as_ref()
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

#[test]
fn dial_gradients_match_their_cpu_evaluation() {
	use rendering::dial_gradient::PencilParameters;

	let size = UVec2::new(200, 200);
	// Gradient space spans 4 units across the render, with the gradient line from (60, 100) to (110, 100)
	let gradient_to_pixels = Affine::new([50., 0., 0., 50., 60., 100.]);

	for dials in [(1., 0.6), (0.6, 0.6), (-0.5, 0.2952), (1., 1.), (0.5, 0.25), (0.3, -0.3)] {
		let parameters = PencilParameters::new(dials.0, dials.1);
		let ramp = peniko::Gradient::new_linear((0., 0.), (1., 0.)).with_stops([Color::from_rgb8(0, 0, 0), Color::from_rgb8(255, 255, 255)]);
		let gradient = vello_gpu::PencilGradient {
			ramp,
			geometry: parameters.to_vello(),
		};

		let draw = |scene: &mut VelloSceneBuilder| scene.fill_viewport(gradient.clone(), Some(gradient_to_pixels));

		let Some(pixels) = render(size, draw) else { return };

		let mut worst = 0_i32;
		for y in (5..195).step_by(10) {
			for x in (5..195).step_by(10) {
				let point = gradient_to_pixels.inverse() * vello_gpu::kurbo::Point::new(x as f64 + 0.5, y as f64 + 0.5);
				let t = parameters.t(glam::DVec2::new(point.x, point.y)).clamp(0., 1.);
				let expected = (t * 255.).round() as i32;
				let actual = pixel(&pixels, size, x, y)[0] as i32;
				worst = worst.max((expected - actual).abs());
			}
		}
		assert!(worst <= 3, "dials {dials:?} differ from the CPU evaluation by up to {worst} levels");
	}
}

/// Renders an opaque white square from (`min`, `min`) to (`max`, `max`) through `effects` with `fill`.
fn render_square_with_effects(effects: Vec<graphic_types::EffectOperation>, fill: f32, min: f64, max: f64, size: UVec2) -> Option<Vec<[u8; 4]>> {
	use rendering::{Render, RenderParams};

	let square = Rect::new(min, min, max, max);

	// The paint overlays' paints, drawn as the renderer does for content placed in the scene's space
	let paints: Vec<graphic_types::Graphic> = (effects.iter())
		.filter_map(|operation| match operation {
			graphic_types::EffectOperation::PaintOverlay { paint, .. } => Some(paint),
			_ => None,
		})
		.map(|paint| graphic_types::fitted_paint(paint, [glam::DVec2::splat(min), glam::DVec2::splat(max)]).unwrap_or_else(|| paint.clone()))
		.collect();
	let mut draw_paint = |index: usize, scene: &mut VelloSceneBuilder| {
		if let Some(paint) = paints.get(index) {
			paint.render_to_vello(scene, DAffine2::IDENTITY, &RenderParams::default().for_pattern());
		}
	};

	let effects = graphic_types::Effects::from_operations(effects);
	render(size, |scene| {
		scene.draw_effects(&effects, fill, Some(square), &mut draw_paint, &mut |scene| {
			scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square)
		});
	})
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
fn shadows_and_glows_fall_where_they_should() {
	use core_types::Color as GraphicColor;
	use glam::DVec2;
	use graphic_types::EffectOperation;

	let size = UVec2::new(128, 128);
	let black = GraphicColor::BLACK;

	// A hard drop shadow offset down and right shows past the square's bottom right corner, beneath it
	let shadow = EffectOperation::DropShadow {
		color: black,
		offset: DVec2::new(10., 10.),
		blur: 0.,
		spread: 0.,
	};
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
		offset: DVec2::new(6., 6.),
		blur: 0.,
		spread: 0.,
	};
	let Some(pixels) = render_square_with_effects(vec![inner_shadow], 1., 30., 80., size) else {
		return;
	};
	assert_eq!(pixel(&pixels, size, 32, 50), [0, 0, 0, 255], "inside the left edge");
	assert_eq!(pixel(&pixels, size, 50, 50), [255, 255, 255, 255], "deep inside");
	assert_eq!(pixel(&pixels, size, 78, 50), [255, 255, 255, 255], "inside the right edge, away from the offset");
	assert_eq!(pixel(&pixels, size, 25, 50)[3], 0, "outside");

	// A glow grown by its spread surrounds the content evenly
	let glow = EffectOperation::OuterGlow {
		color: GraphicColor::from_rgbaf32_unchecked(1., 0., 0., 1.),
		blur: 0.,
		spread: 5.,
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
fn inner_glows_satins_outlines_and_bevels_fall_where_they_should() {
	use core_types::Color as GraphicColor;
	use glam::DVec2;
	use graphic_types::vector_types::vector::style::StrokeAlign;
	use graphic_types::{BevelDirection, BevelStyle, EffectOperation, GlowSource};

	let size = UVec2::new(128, 128);
	let black = GraphicColor::BLACK;
	let render_square = |effect| render_square_with_effects(vec![effect], 1., 30., 80., size);
	let (white, clear) = ([255, 255, 255, 255], 0);

	// A hard inner glow from the edges covers a band inside them, and one from the center covers the rest
	let inner_glow = |source| EffectOperation::InnerGlow {
		color: black,
		blur: 0.,
		choke: 3.,
		source,
	};
	let Some(pixels) = render_square(inner_glow(GlowSource::Edge)) else { return };
	assert_eq!(pixel(&pixels, size, 31, 55), [0, 0, 0, 255], "the edge glows");
	assert_eq!(pixel(&pixels, size, 55, 55), white, "the center doesn't");
	assert_eq!(pixel(&pixels, size, 25, 55)[3], clear, "nothing outside");
	let Some(pixels) = render_square(inner_glow(GlowSource::Center)) else { return };
	assert_eq!(pixel(&pixels, size, 31, 55), white, "the edge doesn't glow");
	assert_eq!(pixel(&pixels, size, 55, 55), [0, 0, 0, 255], "the center does");

	// Satin shades where the two casts of the silhouette disagree, which for a horizontal offset is a band along each side
	let satin = EffectOperation::Satin {
		color: black,
		offset: DVec2::new(10., 0.),
		blur: 0.,
		invert: false,
	};
	let Some(pixels) = render_square(satin) else { return };
	assert_eq!(pixel(&pixels, size, 35, 55), [0, 0, 0, 255], "the left band");
	assert_eq!(pixel(&pixels, size, 55, 55), white, "the middle, where both casts cover");
	assert_eq!(pixel(&pixels, size, 75, 55), [0, 0, 0, 255], "the right band");

	// An outline outside the edge rounds the corners it grows around, and one inside stays within the content
	let outline = |align| EffectOperation::Outline { color: black, weight: 5., align };
	let Some(pixels) = render_square(outline(StrokeAlign::Outside)) else { return };
	assert_eq!(pixel(&pixels, size, 26, 55), [0, 0, 0, 255], "within the weight of the edge");
	assert_eq!(pixel(&pixels, size, 23, 55)[3], clear, "past the weight");
	assert_eq!(pixel(&pixels, size, 27, 27), [0, 0, 0, 255], "near the corner");
	assert!(pixel(&pixels, size, 26, 26)[3] < 128, "beyond the rounded corner, where a square would reach");
	assert_eq!(pixel(&pixels, size, 55, 55), white, "the content shows");
	let Some(pixels) = render_square(outline(StrokeAlign::Inside)) else { return };
	assert_eq!(pixel(&pixels, size, 32, 55), [0, 0, 0, 255], "inside the edge");
	assert_eq!(pixel(&pixels, size, 40, 55), white, "past the weight inside");
	assert_eq!(pixel(&pixels, size, 28, 55)[3], clear, "nothing outside");

	// Lit from the upper left, a raised bevel's top slope catches the highlight and its bottom slope falls into shadow, which swap when it sinks
	let bevel = |style, direction| EffectOperation::BevelAndEmboss {
		style,
		depth: 100.,
		direction,
		size: 5.,
		soften: 0.,
		angle: 120.,
		altitude: 30.,
		highlight_color: GraphicColor::RED,
		shadow_color: GraphicColor::BLUE,
	};
	let reddish = |[red, green, blue, alpha]: [u8; 4]| red > 200 && green < 128 && blue < 128 && alpha > 128;
	let bluish = |[red, green, blue, alpha]: [u8; 4]| blue > 200 && red < 128 && green < 128 && alpha > 128;
	let Some(pixels) = render_square(bevel(BevelStyle::InnerBevel, BevelDirection::Up)) else { return };
	assert!(reddish(pixel(&pixels, size, 55, 32)), "the top slope is highlighted: {:?}", pixel(&pixels, size, 55, 32));
	assert!(bluish(pixel(&pixels, size, 55, 77)), "the bottom slope is shaded: {:?}", pixel(&pixels, size, 55, 77));
	assert_eq!(pixel(&pixels, size, 55, 55), white, "the flat top inside the slopes is unlit");
	assert_eq!(pixel(&pixels, size, 55, 20)[3], clear, "nothing outside");
	let Some(pixels) = render_square(bevel(BevelStyle::InnerBevel, BevelDirection::Down)) else {
		return;
	};
	assert!(bluish(pixel(&pixels, size, 55, 32)), "a sunken top slope is shaded: {:?}", pixel(&pixels, size, 55, 32));
	assert!(reddish(pixel(&pixels, size, 55, 77)), "a sunken bottom slope is highlighted: {:?}", pixel(&pixels, size, 55, 77));

	// An outer bevel's slopes lie outside the content, beneath it
	let Some(pixels) = render_square(bevel(BevelStyle::OuterBevel, BevelDirection::Up)) else { return };
	assert!(reddish(pixel(&pixels, size, 55, 27)), "the top slope outside is highlighted: {:?}", pixel(&pixels, size, 55, 27));
	assert!(bluish(pixel(&pixels, size, 55, 82)), "the bottom slope outside is shaded: {:?}", pixel(&pixels, size, 55, 82));
	assert_eq!(pixel(&pixels, size, 55, 40), white, "the content inside is unlit");
}

#[test]
fn paint_overlays_stretch_unplaced_paints_across_the_content() {
	use core_types::Color as GraphicColor;
	use core_types::blending::BlendMode;
	use core_types::list::Item;
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::vector_types::gradient::{Gradient, GradientStop};
	use graphic_types::{EffectOperation, Graphic};

	let size = UVec2::new(128, 128);
	let overlay = |paint: Graphic| EffectOperation::PaintOverlay {
		paint,
		blend_mode: BlendMode::Normal,
		opacity: 100.,
	};
	let reddish = |[red, green, blue, alpha]: [u8; 4]| red > 200 && green < 64 && blue < 64 && alpha == 255;
	let bluish = |[red, green, blue, alpha]: [u8; 4]| blue > 200 && red < 64 && green < 64 && alpha == 255;

	// A gradient from red to blue runs from the square's left edge to its right edge
	let stops = [(0., GraphicColor::RED), (1., GraphicColor::BLUE)].map(|(position, color)| GradientStop { position, midpoint: 0.5, color });
	let Some(pixels) = render_square_with_effects(vec![overlay(Graphic::from(Gradient::new(stops)))], 1., 30., 80., size) else {
		return;
	};
	assert!(reddish(pixel(&pixels, size, 31, 55)), "the start at the left edge: {:?}", pixel(&pixels, size, 31, 55));
	assert!(bluish(pixel(&pixels, size, 78, 55)), "the end at the right edge: {:?}", pixel(&pixels, size, 78, 55));
	assert_eq!(pixel(&pixels, size, 25, 55)[3], 0, "nothing outside the content");

	// An image of a red pixel beside a blue one covers the square, each pixel half of it
	let image = Image {
		width: 2,
		height: 1,
		data: vec![GraphicColor::RED, GraphicColor::BLUE],
	};
	let Some(pixels) = render_square_with_effects(vec![overlay(Graphic::from(Item::new_from_element(Raster::new_cpu(image))))], 1., 30., 80., size) else {
		return;
	};
	assert!(reddish(pixel(&pixels, size, 35, 40)), "the left half: {:?}", pixel(&pixels, size, 35, 40));
	assert!(bluish(pixel(&pixels, size, 75, 70)), "the right half: {:?}", pixel(&pixels, size, 75, 70));
}

#[test]
fn later_shadows_stack_over_earlier_ones_beneath_the_content() {
	use core_types::Color as GraphicColor;
	use glam::DVec2;
	use graphic_types::EffectOperation;

	let size = UVec2::new(128, 128);
	let shadow = |color, offset: f64| EffectOperation::DropShadow {
		color,
		offset: DVec2::splat(offset),
		blur: 0.,
		spread: 0.,
	};

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

	// A hard shadow around a group whose content is itself a blurred square
	let shadow = Effects::from_operations([EffectOperation::DropShadow {
		color: GraphicColor::BLACK,
		offset: DVec2::new(20., 0.),
		blur: 0.,
		spread: 0.,
	}]);
	let blur = Effects::from_operations([EffectOperation::Blur {
		radius: 3.,
		box_blur: false,
		gamma: false,
	}]);
	let draw = |scene: &mut VelloSceneBuilder| {
		scene.draw_effects(&shadow, 1., Some(square.inflate(12., 12.)), &mut |_, _| {}, &mut |scene| {
			scene.draw_effects(&blur, 1., Some(square), &mut |_, _| {}, &mut |scene| {
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
	use core_types::blending::BlendMode;
	use core_types::list::ATTR_GRADIENT_FORM;
	use core_types::list::{ATTR_EFFECTS, Item};
	use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM};
	use glam::DVec2;
	use graphic_types::adjustment::{Adjustment, LevelsRecord, LevelsSettings};
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::vector_types::gradient::{Gradient, GradientForm, GradientStop};
	use graphic_types::vector_types::vector::style::StrokeAlign;
	use graphic_types::{BevelDirection, BevelStyle, EffectOperation, Effects, GlowSource, Graphic};
	use raster_kernels::evaluate_raster_effects;

	let size = UVec2::new(160, 160);
	let (min, max) = (50, 100);
	let shadow_color = GraphicColor::from_rgbaf32_unchecked(0.1, 0.2, 0.8, 0.75);
	let blur = |radius, box_blur, gamma| EffectOperation::Blur { radius, box_blur, gamma };
	let bevel = |style, direction, size, soften| EffectOperation::BevelAndEmboss {
		style,
		depth: 150.,
		direction,
		size,
		soften,
		angle: 120.,
		altitude: 30.,
		highlight_color: GraphicColor::from_rgbaf32_unchecked(1., 0.9, 0.2, 0.75),
		shadow_color,
	};
	let identity = LevelsRecord {
		shadows: 0.,
		midtones: 1.,
		highlights: 100.,
		output_minimums: 0.,
		output_maximums: 100.,
	};

	let cases = [
		vec![blur(3., false, false)],
		vec![blur(20., false, true)],
		vec![blur(5.5, true, false)],
		vec![EffectOperation::DropShadow {
			color: shadow_color,
			offset: DVec2::new(6.5, -3.),
			blur: 4.,
			spread: 2.,
		}],
		vec![EffectOperation::InnerShadow {
			color: shadow_color,
			offset: DVec2::new(4., 4.),
			blur: 3.,
			spread: 0.,
		}],
		vec![
			EffectOperation::OuterGlow {
				color: shadow_color,
				blur: 6.,
				spread: 3.,
			},
			blur(2., false, false),
		],
		vec![EffectOperation::InnerGlow {
			color: shadow_color,
			blur: 5.,
			choke: 2.,
			source: GlowSource::Edge,
		}],
		vec![EffectOperation::InnerGlow {
			color: shadow_color,
			blur: 8.,
			choke: 0.,
			source: GlowSource::Center,
		}],
		vec![EffectOperation::Satin {
			color: shadow_color,
			offset: DVec2::new(7., 3.),
			blur: 6.,
			invert: true,
		}],
		vec![EffectOperation::Satin {
			color: shadow_color,
			offset: DVec2::new(-4., 9.),
			blur: 3.,
			invert: false,
		}],
		vec![EffectOperation::Outline {
			color: shadow_color,
			weight: 6.,
			align: StrokeAlign::Outside,
		}],
		vec![EffectOperation::Outline {
			color: shadow_color,
			weight: 5.,
			align: StrokeAlign::Inside,
		}],
		vec![EffectOperation::Outline {
			color: shadow_color,
			weight: 9.,
			align: StrokeAlign::Center,
		}],
		vec![bevel(BevelStyle::InnerBevel, BevelDirection::Up, 8., 0.)],
		vec![bevel(BevelStyle::OuterBevel, BevelDirection::Down, 6., 2.)],
		vec![bevel(BevelStyle::Emboss, BevelDirection::Up, 10., 0.)],
		vec![bevel(BevelStyle::PillowEmboss, BevelDirection::Up, 12., 1.)],
		// Bends alpha, so the fill must fade the content before it on both
		vec![EffectOperation::Adjustment(Adjustment::Levels(Box::new(LevelsSettings {
			composite: identity,
			red: identity,
			green: identity,
			blue: identity,
			alpha: LevelsRecord { midtones: 2.2, ..identity },
		})))],
	];

	// Paint overlays over a colored backdrop, their paints placed on their own since the raster and the square fit unplaced paints to different bounds
	let backdrop = EffectOperation::Adjustment(Adjustment::ColorOverlay {
		color: GraphicColor::from_rgbaf32_unchecked(0.3, 0.6, 0.2, 1.),
		blend_mode: BlendMode::Normal,
		opacity: 100.,
	});
	let gradient = |form: GradientForm, transform: DAffine2| {
		let stops = [(0., [0.9, 0.1, 0.4, 1.]), (0.5, [0.2, 0.3, 0.9, 0.5]), (1., [1., 0.9, 0.7, 1.])].map(|(position, [red, green, blue, alpha])| GradientStop {
			position,
			midpoint: 0.5,
			color: GraphicColor::from_rgbaf32_unchecked(red, green, blue, alpha),
		});
		let item = Item::new_from_element(Gradient::new(stops)).with_attribute(ATTR_TRANSFORM, transform);
		Graphic::from(item.with_attribute(ATTR_GRADIENT_FORM, form))
	};
	let linear = gradient(GradientForm::Linear, DAffine2::from_translation(DVec2::new(45., 60.)) * DAffine2::from_scale(DVec2::splat(60.)));
	let radial = gradient(GradientForm::Radial, DAffine2::from_translation(DVec2::splat(75.)) * DAffine2::from_scale(DVec2::splat(30.)));

	// A checkerboard with a pixel for each of the square's, so sampling it needs no interpolation
	let checker = Image {
		width: 50,
		height: 50,
		data: (0..2500)
			.map(|index| match (index % 50 / 5 + index / 50 / 5) % 2 {
				0 => GraphicColor::RED,
				_ => GraphicColor::from_rgbaf32_unchecked(0., 0., 1., 0.5),
			})
			.collect(),
	};
	let checker_placement = DAffine2::from_translation(DVec2::splat(50.)) * DAffine2::from_scale(DVec2::splat(50.));
	let checker = Graphic::from(Item::new_from_element(Raster::new_cpu(checker)).with_attribute(ATTR_TRANSFORM, checker_placement));

	let overlay = |paint: &Graphic, blend_mode, opacity| {
		vec![
			backdrop.clone(),
			EffectOperation::PaintOverlay {
				paint: paint.clone(),
				blend_mode,
				opacity,
			},
		]
	};
	// Hard Mix's threshold and the hue of the gradient's grays turn the slightest rounding into large differences, so they take the checkerboard's exact colors instead
	let ill_conditioned = [BlendMode::HardMix, BlendMode::Hue];
	let mut paint_cases: Vec<_> = (BlendMode::list().into_iter().flatten())
		.filter(|blend_mode| !ill_conditioned.contains(blend_mode))
		.map(|&blend_mode| overlay(&linear, blend_mode, 100.))
		.collect();
	paint_cases.extend(ill_conditioned.map(|blend_mode| overlay(&checker, blend_mode, 100.)));
	paint_cases.extend([
		overlay(&linear, BlendMode::Normal, 40.),
		overlay(&radial, BlendMode::Multiply, 100.),
		overlay(&checker, BlendMode::Screen, 100.),
	]);

	for effects in cases.into_iter().chain(paint_cases) {
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
			let item = Item::new_from_element(Raster::new_cpu(image))
				.with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(size.as_dvec2()))
				.with_attribute(ATTR_EFFECTS, Effects::from_operations(effects.clone()))
				.with_attribute(ATTR_OPACITY_FILL, fill as f64);
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
			// Modes that divide, or land near black where the gamma encoding is steepest, amplify the rounding of the rendered paint's eight bits
			use BlendMode::*;
			let tolerance = match effects.last() {
				Some(EffectOperation::PaintOverlay {
					blend_mode: ColorBurn | LinearBurn | ColorDodge | HardLight | VividLight | LinearLight | PinLight,
					..
				}) => 8,
				_ => 2,
			};
			assert!(worst <= tolerance, "{effects:?} at fill {fill} differs from the GPU by up to {worst} levels");
		}
	}
}

#[test]
fn gpu_adjustment_passes_match_the_cpu_kernels() {
	use core_types::Color as GraphicColor;
	use core_types::list::Item;
	use graphic_types::adjustment::*;
	use graphic_types::raster_types::{Image, Raster};
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

	let levels_record = |shadows, midtones, highlights| LevelsRecord {
		shadows,
		midtones,
		highlights,
		output_minimums: 0.,
		output_maximums: 100.,
	};
	let identity = levels_record(0., 1., 100.);
	let sliders = |hue, saturation, lightness| HueSaturationSliders { hue, saturation, lightness };
	let range = |sliders, falloff_start, range_start, range_end, falloff_end| HueRangeSettings {
		sliders,
		falloff_start,
		range_start,
		range_end,
		falloff_end,
	};
	let zero = sliders(0., 0., 0.);

	// Each run with the most levels it may stray: curves and luma tables by under a level, the cube by its interpolation
	let runs: Vec<(Vec<Adjustment>, u8)> = vec![
		(
			vec![
				Adjustment::Levels(Box::new(LevelsSettings {
					composite: levels_record(10., 1.4, 90.),
					red: identity,
					green: levels_record(5., 0.8, 100.),
					blue: identity,
					alpha: identity,
				})),
				Adjustment::Exposure {
					exposure: 0.5,
					offset: 0.,
					gamma_correction: 1.2,
				},
				Adjustment::Invert,
			],
			2,
		),
		(
			vec![
				Adjustment::HueSaturation(Box::new(HueSaturationSettings {
					master: sliders(25., 30., 5.),
					colorize: false,
					colorize_sliders: zero,
					reds: range(sliders(-15., 40., 0.), 315., 345., 15., 45.),
					yellows: range(zero, 15., 45., 75., 105.),
					greens: range(zero, 75., 105., 135., 165.),
					cyans: range(zero, 135., 165., 195., 225.),
					blues: range(zero, 195., 225., 255., 285.),
					magentas: range(zero, 255., 285., 315., 345.),
				})),
				Adjustment::Desaturate { method: DesaturateMethod::LumaRec601 },
			],
			5,
		),
		(
			vec![Adjustment::GradientMap {
				gradient: Item::new_from_element(graphic_types::vector_types::Gradient::from(vec![
					GraphicColor::from_rgbaf32_unchecked(0.1, 0., 0.4, 1.),
					GraphicColor::from_rgbaf32_unchecked(1., 0.8, 0.2, 1.),
				])),
				reverse: false,
			}],
			2,
		),
		(vec![Adjustment::Vibrance { vibrance: 60., saturation: 20. }], 2),
		// Runs of two and three passes, which hand their colors between textures
		(vec![Adjustment::Vibrance { vibrance: 40., saturation: 0. }, Adjustment::Invert], 3),
		(vec![Adjustment::Invert, Adjustment::Vibrance { vibrance: -30., saturation: 10. }, Adjustment::Invert], 3),
	];

	for (run, tolerance) in runs {
		let mut expected = colors.clone();
		run.iter().for_each(|adjustment| raster_kernels::adjustments::adjust_colors(adjustment, &mut expected));

		// Carried out on the rendered content by an effects group, and on the image's own texture as it's drawn
		let effects = Effects::from_operations(run.iter().cloned().map(EffectOperation::Adjustment));
		let through_effects_group = |scene: &mut VelloSceneBuilder| {
			let bounds = Rect::new(0., 0., size.x as f64, size.y as f64);
			scene.draw_effects(&effects, 1., Some(bounds), &mut |_, _| {}, &mut |scene| {
				scene.draw_raster(raster.clone(), Vec::new(), peniko::ImageSampler::new(), Affine::IDENTITY)
			});
		};
		let on_image_texture = |scene: &mut VelloSceneBuilder| scene.draw_raster(raster.clone(), run.clone(), peniko::ImageSampler::new(), Affine::IDENTITY);
		let paths = [
			("through an effects group", &through_effects_group as &dyn Fn(&mut VelloSceneBuilder)),
			("on the image's texture", &on_image_texture),
		];

		for (path, draw) in paths {
			// The first render bakes any cube as a draft, which rendering the same run again refines
			let Some(_) = render(size, draw) else { return };
			let Some(gpu) = render(size, draw) else { return };

			let mut worst = 0;
			for (index, color) in expected.iter().enumerate() {
				let cpu = color.to_gamma_srgb_channels().map(|channel| (channel * 255.).round() as i32);
				for channel in 0..3 {
					worst = worst.max((cpu[channel] - gpu[index][channel] as i32).unsigned_abs());
				}
			}
			let names = run.iter().map(|adjustment| adjustment.kind().to_string()).collect::<Vec<_>>().join(" + ");
			assert!(worst <= tolerance as u32, "{names} {path} differs from the CPU kernels by up to {worst} levels");
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
		assert!((115..=140).contains(&pixel[0]) && pixel[3] == 255, "pixel {index} is {pixel:?} rather than the gray of its average");
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
fn the_mandelbrot_set_renders_on_the_gpu_like_on_the_cpu() {
	use graphic_types::generator::GeneratorPattern;

	// Its whole unit square across the pixels compared, where the GPU's 32-bit floats may escape a few points on the set's edge at another iteration
	let Some((mismatched, _)) = drawing_differences(&GeneratorPattern::Mandelbrot, 64., glam::DVec2::ZERO) else {
		return;
	};
	assert!(mismatched <= 40, "{mismatched} pixels differ from the CPU's samples");

	// Pixels a billionth of a unit across, too fine for 32-bit floats, on the edge of the set's second largest bulb
	let Some((_, worst)) = drawing_differences(&GeneratorPattern::Mandelbrot, 2_f64.powi(30), glam::DVec2::new(0.25, 0.5)) else {
		return;
	};
	assert!(worst <= 2, "deep zooms differ from the CPU's samples by up to {worst} levels");
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
