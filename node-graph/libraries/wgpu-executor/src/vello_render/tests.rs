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
