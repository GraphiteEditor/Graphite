//! Renders a corpus document at each of its animation times.

use crate::corpus::Entry;
use graphene_cli::export::{ExportSettings, FileType, RenderedFrame};
use graphene_cli::{compile_document, open_document};
use image::RgbaImage;
use std::error::Error;
use std::path::Path;
use std::time::Duration;

/// One image per time in `entry.times`, with straight alpha so transparency regressions show.
pub async fn render(document: &Path, entry: &Entry) -> Result<Vec<RgbaImage>, Box<dyn Error>> {
	let renderer = compile_document(open_document(document, false).await?).await?.into_renderer()?;
	let settings = ExportSettings {
		file_type: FileType::Png,
		scale: entry.scale,
		size: entry.size.map_or((None, None), |(width, height)| (Some(width), Some(height))),
		transparent: true,
		framing: None,
	};
	let settings = renderer.frame(settings).await?;

	let mut images = Vec::with_capacity(entry.times.len());
	for time in &entry.times {
		let RenderedFrame::Raster { data, width, height } = renderer.render(settings, Duration::from_secs_f64(*time)).await? else {
			return Err("a PNG export rendered SVG".into());
		};
		images.push(RgbaImage::from_raw(width, height, data).ok_or("the render has fewer pixels than its size")?);
	}
	Ok(images)
}
