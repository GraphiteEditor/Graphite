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

/// Held while a test uses the shared GPU, since another test's work can delay or intercept a render's readback.
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

/// Renders an opaque white square from (`min`, `min`) to (`max`, `max`) through `effects` with `fill`.
fn render_square_with_effects(effects: Vec<graphic_types::EffectOperation>, fill: f32, min: f64, max: f64, size: UVec2) -> Option<Vec<[u8; 4]>> {
	let square = Rect::new(min, min, max, max);
	let effects = graphic_types::Effects::from_operations(effects);

	render(size, |scene| {
		scene.draw_effects(&effects, fill, Some(square), &mut |scene| scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square));
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
		scene.draw_effects(&shadow, 1., Some(square.inflate(12., 12.)), &mut |scene| {
			scene.draw_effects(&blur, 1., Some(square), &mut |scene| scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square));
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
	use core_types::list::{ATTR_EFFECTS, Item};
	use core_types::{ATTR_OPACITY_FILL, ATTR_TRANSFORM};
	use glam::DVec2;
	use graphic_types::adjustment::{Adjustment, LevelsRecord, LevelsSettings};
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::vector_types::vector::style::StrokeAlign;
	use graphic_types::{BevelDirection, BevelStyle, EffectOperation, Effects, GlowSource};
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
			assert!(worst <= 2, "{effects:?} at fill {fill} differs from the GPU by up to {worst} levels");
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
	];

	for (run, tolerance) in runs {
		let effects = Effects::from_operations(run.iter().cloned().map(EffectOperation::Adjustment));
		let draw = |scene: &mut VelloSceneBuilder| {
			let bounds = Rect::new(0., 0., size.x as f64, size.y as f64);
			scene.draw_effects(&effects, 1., Some(bounds), &mut |scene| {
				scene.draw_raster(raster.clone(), peniko::ImageSampler::new(), Affine::IDENTITY)
			});
		};

		// The first render bakes any cube as a draft, which rendering the same run again refines
		let Some(_) = render(size, draw) else { return };
		let Some(gpu) = render(size, draw) else { return };

		let mut expected = colors.clone();
		run.iter().for_each(|adjustment| raster_kernels::adjustments::adjust_colors(adjustment, &mut expected));

		let mut worst = 0;
		for (index, color) in expected.iter().enumerate() {
			let cpu = color.to_gamma_srgb_channels().map(|channel| (channel * 255.).round() as i32);
			for channel in 0..3 {
				worst = worst.max((cpu[channel] - gpu[index][channel] as i32).unsigned_abs());
			}
		}
		let names = run.iter().map(|adjustment| adjustment.kind().to_string()).collect::<Vec<_>>().join(" + ");
		assert!(worst <= tolerance as u32, "{names} differs from the CPU kernels by up to {worst} levels");
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
	let draw = |scene: &mut VelloSceneBuilder| scene.draw_raster(stripes.clone(), peniko::ImageSampler::new(), Affine::scale(1. / 8.));

	let size = UVec2::new(8, 8);
	let Some(pixels) = render(size, draw) else { return };
	for (index, pixel) in pixels.iter().enumerate() {
		assert!((115..=140).contains(&pixel[0]) && pixel[3] == 255, "pixel {index} is {pixel:?} rather than the gray of its average");
	}
}

#[test]
fn rasters_upload_once_across_scenes_until_modified() {
	use raster_types::{Image, Raster};

	let Some(executor) = executor() else { return };
	let max_texture_size = executor.context().device.limits().max_texture_dimension_2d;
	let raster = Raster::new_cpu(Image::new(8, 8, core_types::Color::WHITE));

	// Each render draws its scene anew, as the editor does whenever the graph runs
	let scene_drawing = |raster: &Raster<raster_types::CPU>| {
		let mut scene = VelloSceneBuilder::new(64, 64, Affine::IDENTITY, max_texture_size);
		scene.draw_raster(raster.clone(), peniko::ImageSampler::new(), Affine::scale(8.));
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
		scene.draw_raster(Raster::new_cpu(Image::new(8, 8, core_types::Color::WHITE)), peniko::ImageSampler::new(), Affine::scale(8.));
		executor.render_vello_scene_on_canvas(&mut canvas, scene).unwrap();
	}
	assert_eq!(canvas.images.textures.len(), 1, "only the texture of pixels that still exist stays");
}
