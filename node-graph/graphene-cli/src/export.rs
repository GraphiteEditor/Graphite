use graph_craft::document::value::{RenderOutputType, TaggedValue, UVec2};
use graph_craft::graphene_compiler::Executor;
use graphene_std::application_io::{ExportFormat, RenderConfig};
use graphene_std::core_types::ops::Convert;
use graphene_std::core_types::transform::Footprint;
use graphene_std::raster_types::{CPU, GPU, Raster};
use interpreted_executor::dynamic_executor::DynamicExecutor;
use std::error::Error;
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
	Svg,
	Png,
	Jpg,
}

pub fn detect_file_type(path: &Path) -> Result<FileType, String> {
	match path.extension().and_then(|s| s.to_str()) {
		Some("svg") => Ok(FileType::Svg),
		Some("png") => Ok(FileType::Png),
		Some("jpg" | "jpeg") => Ok(FileType::Jpg),
		_ => Err("Unsupported file extension. Supported formats: .svg, .png, .jpg".to_string()),
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
				let gpu_raster = Raster::<GPU>::new_gpu(image_texture.texture);
				let cpu_raster: Raster<CPU> = gpu_raster.convert(Footprint::BOUNDLESS, wgpu_executor).await;
				let (data, width, height) = cpu_raster.to_flat_u8();

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
		FileType::Svg => unreachable!("SVG should have been handled in export_document"),
	}

	std::fs::write(&output_path, cursor.into_inner())?;
	Ok(())
}
