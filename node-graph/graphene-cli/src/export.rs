use graph_craft::document::value::{RenderOutputType, TaggedValue, UVec2};
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io::{ExportFormat, RenderConfig, TimingInformation};
use graphene_std::core_types::ops::Convert;
use graphene_std::core_types::transform::Footprint;
use graphene_std::raster_types::{CPU, GPU, Raster};
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
	Gif,
}

pub fn detect_file_type(path: &Path) -> Result<FileType, String> {
	match path.extension().and_then(|s| s.to_str()) {
		Some("svg") => Ok(FileType::Svg),
		Some("png") => Ok(FileType::Png),
		Some("jpg" | "jpeg") => Ok(FileType::Jpg),
		Some("gif") => Ok(FileType::Gif),
		_ => Err("Unsupported file extension. Supported formats: .svg, .png, .jpg, .gif".to_string()),
	}
}

pub async fn export_document(
	executor: &DynamicExecutor,
	wgpu_executor: &wgpu_executor::WgpuExecutor,
	output_path: PathBuf,
	file_type: FileType,
	scale: f64,
	(width, height): (Option<u32>, Option<u32>),
	transparent: bool,
) -> Result<(), Box<dyn Error>> {
	// Determine export format based on file type
	let export_format = match file_type {
		FileType::Svg => ExportFormat::Svg,
		_ => ExportFormat::Raster,
	};

	// Create render config with export settings
	let mut render_config = RenderConfig {
		scale,
		export_format,
		for_export: true,
		..Default::default()
	};

	// Set viewport dimensions if specified
	if let (Some(w), Some(h)) = (width, height) {
		render_config.viewport.resolution = UVec2::new(w, h);
	}

	// Execute the graph
	let result = executor.execute(render_config).await?;

	// Handle the result based on output type
	match result {
		TaggedValue::RenderOutput(output) => match output.data {
			RenderOutputType::Svg { svg, .. } => {
				// Write SVG directly to file
				std::fs::write(&output_path, svg)?;
				log::info!("Exported SVG to: {}", output_path.display());
			}
			RenderOutputType::Texture(image_texture) => {
				// Convert GPU texture to CPU buffer
				let gpu_raster = Raster::<GPU>::new_gpu(image_texture.texture.as_ref().clone());
				let cpu_raster: Raster<CPU> = gpu_raster.convert(Footprint::BOUNDLESS, wgpu_executor).await;
				let (data, width, height) = cpu_raster.to_flat_u8();
				// Explicitly drop texture to make sure it lives long enough
				std::mem::drop(image_texture);

				// Encode and write raster image
				write_raster_image(output_path, file_type, data, width, height, transparent)?;
			}
			RenderOutputType::Buffer { data, width, height } => {
				// Encode and write raster image when buffer is already provided
				write_raster_image(output_path, file_type, data, width, height, transparent)?;
			}
			other => {
				return Err(format!("Unexpected render output type: {:?}. Expected Texture, Buffer for raster export or Svg for SVG export.", other).into());
			}
		},
		other => return Err(format!("Expected RenderOutput, got: {:?}", other).into()),
	}

	Ok(())
}

fn write_raster_image(output_path: PathBuf, file_type: FileType, data: Vec<u8>, width: u32, height: u32, transparent: bool) -> Result<(), Box<dyn Error>> {
	use image::{ImageFormat, RgbaImage};

	let image = RgbaImage::from_raw(width, height, data).ok_or("Failed to create image from buffer")?;

	let mut cursor = Cursor::new(Vec::new());

	match file_type {
		FileType::Png => {
			if transparent {
				image.write_to(&mut cursor, ImageFormat::Png)?;
			} else {
				let image: image::RgbImage = image::DynamicImage::ImageRgba8(image).to_rgb8();
				image.write_to(&mut cursor, ImageFormat::Png)?;
			}
			log::info!("Exported PNG to: {}", output_path.display());
		}
		FileType::Jpg => {
			let image: image::RgbImage = image::DynamicImage::ImageRgba8(image).to_rgb8();
			image.write_to(&mut cursor, ImageFormat::Jpeg)?;
			log::info!("Exported JPG to: {}", output_path.display());
		}
		FileType::Svg | FileType::Gif => unreachable!("SVG and GIF should have been handled in export_document"),
	}

	std::fs::write(&output_path, cursor.into_inner())?;
	Ok(())
}

/// Parameters for GIF animation export
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
		((100.0 / self.fps).round() as u16).max(1)
	}
}

/// Export an animated GIF by rendering multiple frames at different animation times
pub async fn export_gif(
	executor: &DynamicExecutor,
	wgpu_executor: &wgpu_executor::WgpuExecutor,
	output_path: PathBuf,
	scale: f64,
	(width, height): (Option<u32>, Option<u32>),
	animation: AnimationParams,
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

		// Create render config with animation time
		let mut render_config = RenderConfig {
			scale,
			export_format: ExportFormat::Raster,
			for_export: true,
			time: TimingInformation {
				time: animation_time.as_secs_f64(),
				animation_time,
			},
			..Default::default()
		};

		// Set viewport dimensions if specified
		if let (Some(w), Some(h)) = (width, height) {
			render_config.viewport.resolution = UVec2::new(w, h);
		}

		// Execute the graph for this frame
		let result = executor.execute(render_config).await?;

		// Extract RGBA data from result
		let (data, img_width, img_height) = match result {
			TaggedValue::RenderOutput(output) => match output.data {
				RenderOutputType::Texture(image_texture) => {
					let gpu_raster = Raster::<GPU>::new_gpu(image_texture.texture.as_ref().clone());
					let cpu_raster: Raster<CPU> = gpu_raster.convert(Footprint::BOUNDLESS, wgpu_executor).await;
					// Explicitly drop texture to make sure it lives long enough
					std::mem::drop(image_texture);
					cpu_raster.to_flat_u8()
				}
				RenderOutputType::Buffer { data, width, height } => (data, width, height),
				other => {
					return Err(format!("Unexpected render output type for GIF frame: {:?}. Expected Texture or Buffer.", other).into());
				}
			},
			other => return Err(format!("Expected RenderOutput for GIF frame, got: {:?}", other).into()),
		};

		// Create image frame
		let image = RgbaImage::from_raw(img_width, img_height, data).ok_or("Failed to create image from buffer")?;

		// Create GIF frame with delay (delay is in 10ms units)
		let frame = Frame::from_parts(image, 0, 0, image::Delay::from_saturating_duration(std::time::Duration::from_millis(frame_delay as u64 * 10)));

		encoder.encode_frame(frame)?;
	}

	// Clear the progress line
	eprintln!();

	log::info!("Exported GIF to: {}", output_path.display());
	Ok(())
}
