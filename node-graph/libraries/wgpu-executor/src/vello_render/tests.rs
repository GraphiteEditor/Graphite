//! Renders scenes that stress Vello GPU on the machine's GPU, skipping when none is available.

use crate::WgpuExecutor;
use glam::{DAffine2, UVec2};
use rendering::scene::Scene;
use vello_gpu::kurbo::{Affine, BezPath, Line, Rect, Stroke};
use vello_gpu::peniko::{self, Color, Fill};

/// The executor every test shares, or `None` without a GPU.
fn executor() -> Option<&'static WgpuExecutor> {
	static EXECUTOR: std::sync::OnceLock<Option<WgpuExecutor>> = std::sync::OnceLock::new();
	EXECUTOR.get_or_init(|| futures::executor::block_on(WgpuExecutor::new())).as_ref()
}

/// Renders `scene` into straight-alpha RGBA8 pixels, or `None` without a GPU.
fn render(scene: &Scene, size: UVec2) -> Option<Vec<[u8; 4]>> {
	render_timed(scene, size).map(|(pixels, _)| pixels)
}

/// Renders `scene` into straight-alpha RGBA8 pixels, with how long the render and its readback took, or `None` without a GPU.
/// Renders take turns, since reading one back waits for whatever else the shared GPU is working on.
fn render_timed(scene: &Scene, size: UVec2) -> Option<(Vec<[u8; 4]>, std::time::Duration)> {
	static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

	let executor = executor()?;
	let _turn = TURN.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
	let start = std::time::Instant::now();

	let texture = match executor.render_vello_scene(scene, DAffine2::IDENTITY, size) {
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

#[test]
fn dense_strokes_render() {
	let size = UVec2::new(2048, 2048);
	let mut scene = Scene::new();
	for i in 0..40_000 {
		let y = (i as f64 * 0.0511) % 2048.;
		scene.stroke(&Stroke::new(1.), Affine::IDENTITY, RED, None, &Line::new((0., y), (2048., y + 3.)));
	}

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 1024, 1024)[3], 255);
}

#[test]
fn huge_circle_edge_lands_on_the_right_pixel() {
	let size = UVec2::new(256, 256);
	let mut scene = Scene::new();
	for radius in [1e6, 1e9, 1e12] {
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((128. - radius, 128.), radius));
	}

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 120, 128)[3], 255, "inside");
	assert_eq!(pixel(&pixels, size, 136, 128)[3], 0, "outside");
}

#[test]
fn huge_curves_through_the_viewport_render_quickly() {
	let size = UVec2::new(256, 256);
	for radius in [1e13, 1e14, 1e15] {
		let mut scene = Scene::new();
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((128. - radius, 128.), radius));
		scene.stroke(&Stroke::new(4.), Affine::IDENTITY, RED, None, &circle((200. + radius, 128.), radius));

		let Some((pixels, duration)) = render_timed(&scene, size) else { return };
		println!("Radius {radius}: {duration:?}");
		assert_eq!(pixel(&pixels, size, 120, 128)[3], 255, "inside at radius {radius}");
		assert_eq!(pixel(&pixels, size, 160, 128)[3], 0, "outside at radius {radius}");
		assert_eq!(pixel(&pixels, size, 200, 128)[3], 255, "stroke at radius {radius}");
	}
}

#[test]
fn astronomical_fills_render_quickly() {
	let size = UVec2::new(256, 256);
	for radius in [1e18, 1e30, 1e100, 1e300] {
		let mut scene = Scene::new();
		scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &circle((0., 0.), radius));

		let Some((pixels, duration)) = render_timed(&scene, size) else { return };
		println!("Fill radius {radius}: {duration:?}");
		assert!(duration.as_secs_f64() < 2., "radius {radius} took {duration:?}");
		assert_eq!(pixel(&pixels, size, 128, 128)[3], 255, "inside at radius {radius}");
	}
}

#[test]
fn astronomical_strokes_render_quickly() {
	let size = UVec2::new(256, 256);
	for radius in [1e18, 1e30, 1e100, 1e300] {
		let mut scene = Scene::new();
		scene.stroke(&Stroke::new(1e3), Affine::IDENTITY, RED, None, &circle((0., 0.), radius));

		let Some((pixels, duration)) = render_timed(&scene, size) else { return };
		println!("Stroke radius {radius}: {duration:?}");
		assert!(duration.as_secs_f64() < 2., "radius {radius} took {duration:?}");
		assert_eq!(pixel(&pixels, size, 128, 128)[3], 0, "far inside at radius {radius}");
	}
}

#[test]
fn enormous_line_renders() {
	let size = UVec2::new(256, 256);
	let mut scene = Scene::new();
	scene.stroke(&Stroke::new(4.), Affine::IDENTITY, RED, None, &Line::new((-1e30, 100.), (1e30, 100.)));

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 128, 100)[3], 255);
}

#[test]
fn infinite_line_renders() {
	let size = UVec2::new(256, 256);
	let mut scene = Scene::new();
	let mut path = BezPath::new();
	path.move_to((-5., f64::INFINITY));
	path.line_to((5., f64::NEG_INFINITY));
	scene.stroke(&Stroke::new(4.), Affine::translate((128., 0.)), RED, None, &path);

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 128, 128)[3], 255);
}

#[test]
fn nan_points_render_nothing_wrong() {
	let size = UVec2::new(256, 256);
	let mut scene = Scene::new();
	let mut path = BezPath::new();
	path.move_to((10., 10.));
	path.line_to((f64::NAN, 100.));
	path.line_to((100., 100.));
	path.close_path();
	scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &path);
	scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(200., 200., 250., 250.));

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 225, 225)[3], 255);
}

#[test]
fn many_gradients_render() {
	let size = UVec2::new(1024, 1024);
	let mut scene = Scene::new();
	for i in 0..250_000_u32 {
		let (x, y) = ((i % 500) as f64 * 2., (i / 500) as f64 * 2.);
		let gradient = peniko::Gradient::new_linear((x, y), (x + 2., y)).with_stops([Color::from_rgb8((i % 256) as u8, 0, 0), Color::from_rgb8(0, (i / 256 % 256) as u8, 255)]);
		scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &Rect::new(x, y, x + 2., y + 2.));
	}

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 500, 500)[3], 255);
}

#[test]
fn deep_layer_nesting_renders() {
	let size = UVec2::new(256, 256);
	let mut scene = Scene::new();
	for _ in 0..2000 {
		scene.push_layer(Fill::NonZero, peniko::Mix::Multiply, 1., Affine::IDENTITY, &Rect::new(0., 0., 256., 256.));
	}
	scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(0., 0., 256., 256.));
	for _ in 0..2000 {
		scene.pop_layer();
	}

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 128, 128)[3], 255);
}

#[test]
fn render_larger_than_the_texture_limit_is_refused() {
	let mut scene = Scene::new();
	scene.fill(Fill::NonZero, Affine::IDENTITY, RED, None, &Rect::new(0., 0., 1e6, 1e6));

	let Some(executor) = executor() else { return };
	let limit = executor.context().device.limits().max_texture_dimension_2d;
	let size = UVec2::new(limit + 100, 16);
	let texture = executor.render_vello_scene(&scene, DAffine2::IDENTITY, size);
	assert!(texture.is_err());
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

	let mut scene = Scene::new();
	scene.draw_image(&image, Affine::scale_non_uniform(256. / width as f64, 64.));

	let Some(pixels) = render(&scene, size) else { return };
	assert_eq!(pixel(&pixels, size, 128, 128)[3], 255);
}

#[test]
fn degenerate_strokes_render() {
	let size = UVec2::new(256, 256);
	let mut scene = Scene::new();
	scene.stroke(&Stroke::new(1e20), Affine::IDENTITY, RED, None, &Line::new((10., 10.), (20., 20.)));
	scene.stroke(&Stroke::new(f64::NAN), Affine::IDENTITY, RED, None, &Line::new((10., 10.), (20., 20.)));
	scene.stroke(&Stroke::new(5.), Affine::scale(0.), RED, None, &Line::new((10., 10.), (20., 20.)));
	scene.stroke(&Stroke::new(5.), Affine::scale(f64::INFINITY), RED, None, &Line::new((10., 10.), (20., 20.)));
	scene.stroke(&Stroke::new(5.), Affine::IDENTITY, RED, None, &Line::new((50., 50.), (50., 50.)));

	let Some(_) = render(&scene, size) else { return };
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

		let mut scene = Scene::new();
		scene.fill_viewport(gradient, Some(gradient_to_pixels));

		let Some(pixels) = render(&scene, size) else { return };

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
	let square = Rect::new(min, min, max, max);

	let mut scene = Scene::new();
	scene.push_effects(graphic_types::Effects::from_operations(effects), fill, Some(square));
	scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square);
	scene.pop_layer();

	render(&scene, size)
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
	let mut scene = Scene::new();
	let shadow = EffectOperation::DropShadow {
		color: GraphicColor::BLACK,
		offset: DVec2::new(20., 0.),
		blur: 0.,
		spread: 0.,
	};
	scene.push_effects(Effects::from_operations([shadow]), 1., Some(square.inflate(12., 12.)));
	let blur = EffectOperation::Blur {
		radius: 3.,
		box_blur: false,
		gamma: false,
	};
	scene.push_effects(Effects::from_operations([blur]), 1., Some(square));
	scene.fill(Fill::NonZero, Affine::IDENTITY, Color::WHITE, None, &square);
	scene.pop_layer();
	scene.pop_layer();

	let Some(pixels) = render(&scene, size) else { return };
	let edge = pixel(&pixels, size, 30, 55);
	assert!((100..=155).contains(&edge[3]), "the blur softens the edge to half coverage, not {edge:?}");
	// Ten pixels past the content's blurred edge, where the shadow is cast from ten pixels inside it
	let shadow = pixel(&pixels, size, 90, 55);
	assert!(shadow[3] >= 250 && shadow[0] <= 5, "the shadow is cast by the blurred group, not {shadow:?}");
}

#[test]
fn rasters_upload_once_across_scenes_until_modified() {
	use graphic_types::raster_types::{Image, Raster};

	let Some(executor) = executor() else { return };
	let size = UVec2::new(64, 64);
	let raster = Raster::new_cpu(Image::new(8, 8, core_types::Color::WHITE));

	// Each render records its scene anew, as the editor does whenever the graph runs
	let scene_drawing = |raster: &Raster<graphic_types::raster_types::CPU>| {
		let mut scene = Scene::new();
		scene.draw_raster(raster.clone(), peniko::ImageSampler::new(), Affine::scale(8.));
		scene
	};

	let mut canvas = crate::VelloCanvas::default();
	for _ in 0..3 {
		executor.render_vello_scene_on_canvas(&mut canvas, &scene_drawing(&raster), DAffine2::IDENTITY, size).unwrap();
	}
	assert_eq!(canvas.images.textures.len(), 1, "a clone of the same pixels reuses their texture");

	let mut modified = raster.clone();
	modified.data_mut().data[0] = core_types::Color::BLACK;
	executor.render_vello_scene_on_canvas(&mut canvas, &scene_drawing(&modified), DAffine2::IDENTITY, size).unwrap();
	assert_eq!(canvas.images.textures.len(), 2, "modified pixels get a texture of their own");
}
