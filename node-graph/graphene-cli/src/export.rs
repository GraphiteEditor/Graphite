use crate::TokioSpawner;
use graph_craft::document::value::{DAffine2, DVec2, RenderOutputType, TaggedValue, UVec2};
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io::{ExportFormat, RenderConfig, TimingInformation};
use graphene_std::core_types::ops::ConvertAsync;
use graphene_std::core_types::transform::Footprint;
use graphene_std::raster_types::{CPU, GPU, Raster};
use graphene_std::runtime::AsyncRuntime;
use graphene_std::vector::{no_paths, paths_from_item_attributes};
use graphene_std::{Cover, Coverage};
use graphene_std::{NodeError, Poll};
use image::ImageFormat;
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::error::Error;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
	Svg,
	Png,
	Jpg,
	Webp,
	Tiff,
	Bmp,
	Tga,
	Ico,
	Gif,
}

impl FileType {
	/// The image format of a file type encoded from a single image, which excludes SVG and GIF.
	fn raster_format(self) -> Option<ImageFormat> {
		match self {
			FileType::Png => Some(ImageFormat::Png),
			FileType::Jpg => Some(ImageFormat::Jpeg),
			FileType::Webp => Some(ImageFormat::WebP),
			FileType::Tiff => Some(ImageFormat::Tiff),
			FileType::Bmp => Some(ImageFormat::Bmp),
			FileType::Tga => Some(ImageFormat::Tga),
			FileType::Ico => Some(ImageFormat::Ico),
			FileType::Svg | FileType::Gif => None,
		}
	}
}

pub fn detect_file_type(path: &Path) -> Result<FileType, String> {
	match path.extension().and_then(|s| s.to_str()) {
		Some("svg") => Ok(FileType::Svg),
		Some("png") => Ok(FileType::Png),
		Some("jpg" | "jpeg") => Ok(FileType::Jpg),
		Some("webp") => Ok(FileType::Webp),
		Some("tif" | "tiff") => Ok(FileType::Tiff),
		Some("bmp") => Ok(FileType::Bmp),
		Some("tga") => Ok(FileType::Tga),
		Some("ico") => Ok(FileType::Ico),
		Some("gif") => Ok(FileType::Gif),
		_ => Err("Unsupported file extension. Supported formats: .svg, .png, .jpg, .webp, .tiff, .bmp, .tga, .ico, .gif".to_string()),
	}
}

/// One rendered frame of the document, as SVG text or as 8-bit RGBA pixels with straight alpha.
enum RenderedFrame {
	Svg(String),
	Raster { data: Vec<u8>, width: u32, height: u32 },
}

/// The output settings shared by every kind of export.
#[derive(Debug, Clone, Copy)]
pub struct ExportSettings {
	pub file_type: FileType,
	pub scale: f64,
	/// Output width and height in pixels, used when both are given
	pub size: (Option<u32>, Option<u32>),
	/// Whether raster formats that can keep transparency do so, instead of compositing onto white
	pub transparent: bool,
	/// The document region to render, which [`ExportSettings::framed`] fits to the artwork when no size is given
	pub framing: Option<Framing>,
}

/// A document-space region mapped onto the output pixels.
#[derive(Debug, Clone, Copy)]
pub struct Framing {
	pub transform: DAffine2,
	pub resolution: UVec2,
}

impl ExportSettings {
	/// Frame the artwork as it is at time zero, unless a size was given.
	/// Every frame of an animation shares this framing, so they all have one size.
	pub async fn framed(mut self, executor: &DynamicExecutor, runtime: &AsyncRuntime, spawner: &TokioSpawner) -> Result<Self, Box<dyn Error>> {
		if self.size.0.is_some() && self.size.1.is_some() {
			return Ok(self);
		}

		let [min, max] = artwork_bounds(executor, runtime, spawner).await?.ok_or("The document has no artwork to export")?;
		let resolution = ((max - min) * self.scale).round().max(DVec2::ONE).as_uvec2();
		self.framing = Some(Framing {
			transform: DAffine2::from_translation(min).inverse(),
			resolution,
		});
		Ok(self)
	}
}

/// The document-space bounds of every layer including its stroke, artboards included, from the metadata of an SVG render.
/// Content an artboard clips away still counts, since this has no layer tree to tell which artboard holds it.
async fn artwork_bounds(executor: &DynamicExecutor, runtime: &AsyncRuntime, spawner: &TokioSpawner) -> Result<Option<[DVec2; 2]>, Box<dyn Error>> {
	// An identity viewport, since the default one is degenerate and its transforms come out NaN
	let render_config = RenderConfig {
		viewport: Footprint {
			transform: DAffine2::IDENTITY,
			resolution: UVec2::ONE,
			..Default::default()
		},
		scale: 1.,
		export_format: ExportFormat::Svg,
		for_export: true,
		..Default::default()
	};
	let TaggedValue::RenderOutput(output) = execute_until_settled(executor, runtime, spawner, render_config).await? else {
		return Err("The document did not render".into());
	};

	let metadata = &output.metadata;
	let layer_bounds = metadata.click_targets.iter().filter_map(|(layer, click_targets)| {
		let footprint = metadata.upstream_footprints.get(layer).map(|footprint| footprint.transform).unwrap_or_default();
		let local_transform = metadata.local_transforms.get(layer).copied().unwrap_or_default();
		let transform = footprint * local_transform;

		// A stroke extends past the path a click target outlines, so a vector layer also measures its rendered geometry.
		// Metadata only records the first item of a layer, so the other items count through their click targets, without their strokes.
		let stroked = metadata.vector_data.get(layer).and_then(|item| {
			let appearance = metadata.appearance_attributes.get(layer);
			let stroke = appearance.and_then(|appearance| appearance.first_coverage_of(Cover::Stroke)).map(Coverage::stroke_params);
			let paths = paths_from_item_attributes(item.as_ref()).unwrap_or_else(|| no_paths());
			item.element().stroke_inclusive_bounding_box_with_transform(paths, transform, stroke.as_ref())
		});

		click_targets
			.iter()
			.filter_map(|click_target| click_target.bounding_box_with_transform(transform))
			.chain(stroked)
			.reduce(combine_bounds)
	});
	Ok(layer_bounds.reduce(combine_bounds))
}

fn combine_bounds([a_min, a_max]: [DVec2; 2], [b_min, b_max]: [DVec2; 2]) -> [DVec2; 2] {
	[a_min.min(b_min), a_max.max(b_max)]
}

/// Renders the document at the given animation time, as SVG text for an SVG export and as pixels for every other file type.
async fn render_frame(
	executor: &DynamicExecutor,
	wgpu_executor: &wgpu_executor::WgpuExecutor,
	settings: ExportSettings,
	animation_time: Duration,
	runtime: &AsyncRuntime,
	spawner: &TokioSpawner,
) -> Result<RenderedFrame, Box<dyn Error>> {
	let ExportSettings {
		file_type,
		scale,
		size: (width, height),
		..
	} = settings;
	let export_format = if file_type == FileType::Svg { ExportFormat::Svg } else { ExportFormat::Raster };

	// Create render config with export settings and the animation time
	let mut render_config = RenderConfig {
		scale,
		export_format,
		for_export: true,
		time: TimingInformation {
			time: animation_time.as_secs_f64(),
			animation_time,
		},
		..Default::default()
	};

	if let Some(framing) = settings.framing {
		render_config.viewport.transform = framing.transform;
		render_config.viewport.resolution = framing.resolution;
	} else if let (Some(w), Some(h)) = (width, height) {
		render_config.viewport.resolution = UVec2::new(w, h);
	}

	// Execute the graph and extract its output
	match execute_until_settled(executor, runtime, spawner, render_config).await? {
		TaggedValue::RenderOutput(output) => match output.data {
			RenderOutputType::Svg { svg, .. } => Ok(RenderedFrame::Svg(svg)),
			RenderOutputType::Texture(texture) => {
				// Convert GPU texture to CPU buffer
				let gpu_raster = Raster::<GPU>::new_gpu(texture);
				let cpu_raster: Raster<CPU> = gpu_raster.convert(Footprint::BOUNDLESS, wgpu_executor.clone()).await;
				let (data, width, height) = cpu_raster.to_flat_u8();
				Ok(RenderedFrame::Raster { data, width, height })
			}
			RenderOutputType::Buffer { data, width, height } => Ok(RenderedFrame::Raster { data, width, height }),
			#[cfg(target_family = "wasm")]
			other => Err(format!("Unexpected render output type: {other:?}. Expected Texture, Buffer for raster export or Svg for SVG export.").into()),
		},
		other => Err(format!("Expected RenderOutput, got: {other:?}").into()),
	}
}

/// Writes a rendered frame to a file of the export's type.
fn write_frame(path: &Path, settings: ExportSettings, frame: RenderedFrame) -> Result<(), Box<dyn Error>> {
	let contents = match frame {
		RenderedFrame::Svg(svg) => svg.into_bytes(),
		RenderedFrame::Raster { data, width, height } => encode_raster(settings.file_type, data, width, height, settings.transparent)?,
	};
	std::fs::write(path, contents)?;
	Ok(())
}

/// Encodes 8-bit RGBA pixels with straight alpha as a file of the given raster type.
/// Unless transparency is kept, which JPG can't do, the image is composited onto white.
fn encode_raster(file_type: FileType, data: Vec<u8>, width: u32, height: u32, transparent: bool) -> Result<Vec<u8>, Box<dyn Error>> {
	use image::buffer::ConvertBuffer;
	use image::codecs::tga::TgaEncoder;
	use image::{DynamicImage, RgbaImage};

	let Some(format) = file_type.raster_format() else {
		return Err(format!("{file_type:?} can't be encoded from a single image").into());
	};
	if file_type == FileType::Ico && (width > 256 || height > 256) {
		return Err("An ICO image can be at most 256 pixels wide and tall. Lower the scale or the output size.".into());
	}

	let mut image = RgbaImage::from_raw(width, height, data).ok_or("Failed to create image from buffer")?;

	let image = if transparent && file_type != FileType::Jpg {
		DynamicImage::ImageRgba8(image)
	} else {
		for pixel in image.pixels_mut() {
			let [r, g, b, a] = pixel.0;
			let alpha = a as f32 / 255.;
			let blend = |channel: u8| (channel as f32 * alpha + 255. * (1. - alpha)).round() as u8;
			*pixel = image::Rgba([blend(r), blend(g), blend(b), 255]);
		}

		// An ICO only holds images with an alpha channel, so it keeps the now-opaque one
		if file_type == FileType::Ico {
			DynamicImage::ImageRgba8(image)
		} else {
			DynamicImage::ImageRgb8(image.convert())
		}
	};

	let mut cursor = Cursor::new(Vec::new());

	// TGA is left uncompressed because the image crate's compressed runs continue from one row into the next, which many readers reject
	if file_type == FileType::Tga {
		image.write_with_encoder(TgaEncoder::new(&mut cursor).disable_rle())?;
	} else {
		image.write_to(&mut cursor, format)?;
	}

	Ok(cursor.into_inner())
}

pub async fn export_document(
	executor: &DynamicExecutor,
	wgpu_executor: &wgpu_executor::WgpuExecutor,
	output_path: PathBuf,
	settings: ExportSettings,
	runtime: &AsyncRuntime,
	spawner: &TokioSpawner,
) -> Result<(), Box<dyn Error>> {
	let frame = render_frame(executor, wgpu_executor, settings, Duration::ZERO, runtime, spawner).await?;
	write_frame(&output_path, settings, frame)?;

	log::info!("Exported to: {}", output_path.display());
	Ok(())
}

/// Parameters for exporting an animation as a GIF or an image sequence.
#[derive(Debug, Clone, Copy)]
pub struct AnimationParams {
	/// Frames per second
	pub fps: f64,
	/// Total number of frames to render
	pub frames: u32,
}

impl AnimationParams {
	/// Create animation parameters from fps and either frame count or duration
	pub fn new(fps: f64, frames: Option<u32>, duration: Option<f64>) -> Self {
		let frames = match (frames, duration) {
			// Duration takes precedence if both provided
			(_, Some(dur)) => (dur * fps).round() as u32,
			(Some(f), None) => f,
			// Default to 1 frame if neither provided
			(None, None) => 1,
		};
		Self { fps, frames }
	}

	/// Get the frame delay in centiseconds (GIF uses 10ms units)
	pub fn frame_delay_centiseconds(&self) -> u16 {
		((100. / self.fps).round() as u16).max(1)
	}
}

/// Export an animated GIF by rendering multiple frames at different animation times
pub async fn export_gif(
	executor: &DynamicExecutor,
	wgpu_executor: &wgpu_executor::WgpuExecutor,
	output_path: PathBuf,
	settings: ExportSettings,
	animation: AnimationParams,
	runtime: &AsyncRuntime,
	spawner: &TokioSpawner,
) -> Result<(), Box<dyn Error>> {
	use image::codecs::gif::{GifEncoder, Repeat};
	use image::{Frame, RgbaImage};
	use std::fs::File;

	log::info!("Exporting GIF: {} frames at {} fps", animation.frames, animation.fps);

	let file = File::create(&output_path)?;
	let mut encoder = GifEncoder::new(file);
	encoder.set_repeat(Repeat::Infinite)?;

	let frame_delay = animation.frame_delay_centiseconds();

	for frame_idx in 0..animation.frames {
		let animation_time = Duration::from_secs_f64(frame_idx as f64 / animation.fps);

		// Print progress to stderr (overwrites previous line)
		eprint!("\rRendering frame {}/{}...", frame_idx + 1, animation.frames);

		log::debug!("Rendering frame {}/{} at time {:?}", frame_idx + 1, animation.frames, animation_time);

		let RenderedFrame::Raster { data, width, height } = render_frame(executor, wgpu_executor, settings, animation_time, runtime, spawner).await? else {
			return Err("A GIF frame must render as pixels".into());
		};

		// Create image frame
		let image = RgbaImage::from_raw(width, height, data).ok_or("Failed to create image from buffer")?;

		// Create GIF frame with delay (delay is in 10ms units)
		let frame = Frame::from_parts(image, 0, 0, image::Delay::from_saturating_duration(std::time::Duration::from_millis(frame_delay as u64 * 10)));

		encoder.encode_frame(frame)?;
	}

	// Clear the progress line
	eprintln!();

	log::info!("Exported GIF to: {}", output_path.display());
	Ok(())
}

/// Export an animation as one numbered file per frame beside the output path (`name_000.png`, `name_001.png`, ..., zero-padded to fit the frame count).
pub async fn export_image_sequence(
	executor: &DynamicExecutor,
	wgpu_executor: &wgpu_executor::WgpuExecutor,
	output_path: PathBuf,
	settings: ExportSettings,
	animation: AnimationParams,
	runtime: &AsyncRuntime,
	spawner: &TokioSpawner,
) -> Result<(), Box<dyn Error>> {
	log::info!("Exporting image sequence: {} frames at {} fps", animation.frames, animation.fps);

	let stem = output_path.file_stem().and_then(|stem| stem.to_str()).ok_or("The output path needs a file name")?;
	let extension = output_path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
	let directory = output_path.parent().map(Path::to_path_buf).unwrap_or_default();
	std::fs::create_dir_all(&directory)?;
	let digits = animation.frames.saturating_sub(1).max(1).ilog10() as usize + 1;

	for frame_idx in 0..animation.frames {
		let animation_time = Duration::from_secs_f64(frame_idx as f64 / animation.fps);

		// Print progress to stderr (overwrites previous line)
		eprint!("\rRendering frame {}/{}...", frame_idx + 1, animation.frames);

		let frame = render_frame(executor, wgpu_executor, settings, animation_time, runtime, spawner).await?;
		let frame_path = directory.join(format!("{stem}_{frame_idx:0digits$}.{extension}"));
		write_frame(&frame_path, settings, frame)?;
	}

	// Clear the progress line
	eprintln!();

	log::info!("Exported {} frames to: {}", animation.frames, directory.display());
	Ok(())
}

async fn execute_until_settled(executor: &DynamicExecutor, runtime: &AsyncRuntime, spawner: &TokioSpawner, render_config: RenderConfig) -> Result<TaggedValue, Box<dyn Error>> {
	let execute = || executor.execute(render_config.into_context_with_generations(runtime.snapshot()));
	let mut frame = execute()?;
	while spawner.join().await {
		frame = execute()?;
	}
	match frame {
		Poll::Fallback(boxed) => {
			let (value, error) = *boxed;
			log::warn!("Exported fallback value due to node error: {}", messages(&error));
			Ok(value)
		}
		Poll::Error(error) => Err(format!("Node graph failed: {}", messages(&error)).into()),
		frame => frame.value().ok_or_else(|| "Node graph produced no frame".into()),
	}
}

fn messages(error: &NodeError) -> String {
	let mut text = error.to_string();
	for upstream in error.upstream() {
		text.push('\n');
		text.push_str(&messages(upstream));
	}
	text
}

#[cfg(test)]
mod tests {
	use super::*;

	const RASTER_FILE_TYPES: [FileType; 7] = [FileType::Png, FileType::Jpg, FileType::Webp, FileType::Tiff, FileType::Bmp, FileType::Tga, FileType::Ico];

	#[test]
	fn every_raster_file_type_encodes_an_image_that_reads_back() {
		// Opaque red, then fully transparent
		let pixels = vec![255, 0, 0, 255, 0, 0, 0, 0];

		for file_type in RASTER_FILE_TYPES {
			for transparent in [true, false] {
				let encoded = encode_raster(file_type, pixels.clone(), 2, 1, transparent).unwrap_or_else(|error| panic!("{file_type:?}: {error}"));
				let format = file_type.raster_format().unwrap();
				let decoded = image::load_from_memory_with_format(&encoded, format).unwrap_or_else(|error| panic!("{file_type:?} should read back: {error}"));
				let decoded = decoded.to_rgba8();
				assert_eq!(decoded.dimensions(), (2, 1), "{file_type:?}");

				// The transparent pixel stays transparent only when asked to and possible, and otherwise lands on white
				let pixel = decoded.get_pixel(1, 0).0;
				if transparent && file_type != FileType::Jpg {
					assert_eq!(pixel[3], 0, "{file_type:?}");
				} else {
					assert!(pixel.iter().all(|&channel| channel > 250), "{file_type:?} with transparency {transparent}: {pixel:?}");
				}
			}
		}
	}

	#[test]
	fn file_types_that_cannot_hold_the_image_say_so() {
		assert!(encode_raster(FileType::Svg, vec![0; 4], 1, 1, true).is_err());
		assert!(encode_raster(FileType::Gif, vec![0; 4], 1, 1, true).is_err());
		assert!(encode_raster(FileType::Png, vec![0; 4], 2, 2, true).is_err());

		let too_large = encode_raster(FileType::Ico, vec![0; 257 * 4], 257, 1, true).unwrap_err().to_string();
		assert!(too_large.contains("256 pixels"), "{too_large}");
		assert!(encode_raster(FileType::Ico, vec![0; 256 * 4], 256, 1, true).is_ok());
	}

	#[test]
	fn tga_is_written_uncompressed() {
		// Byte 2 of a TGA header is its image type, where 2 means uncompressed true color
		let encoded = encode_raster(FileType::Tga, vec![255, 0, 0, 255, 0, 0, 0, 0], 2, 1, true).unwrap();
		assert_eq!(encoded[2], 2);
	}

	#[test]
	fn file_types_are_detected_from_the_extension() {
		assert_eq!(detect_file_type(Path::new("frame.tif")), Ok(FileType::Tiff));
		assert_eq!(detect_file_type(Path::new("frame.jpeg")), Ok(FileType::Jpg));
		assert_eq!(detect_file_type(Path::new("frame.webp")), Ok(FileType::Webp));
		assert!(detect_file_type(Path::new("frame.exr")).is_err());
		assert!(detect_file_type(Path::new("frame")).is_err());
	}
}
