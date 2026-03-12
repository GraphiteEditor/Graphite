use krilla::page::PageSettings;
use krilla_svg::{SurfaceExt, SvgSettings};

/// Convert an SVG string to PDF bytes using krilla and krilla-svg.

pub fn svg_to_pdf(svg: &str, page_width: Option<f32>, page_height: Option<f32>) -> Result<Vec<u8>, String> {
	let options = usvg_045::Options::default();
	let tree = usvg_045::Tree::from_str(svg, &options).map_err(|e| format!("Failed to parse SVG for PDF conversion: {e}"))?;

	let page_width = page_width.unwrap_or(tree.size().width() as f32).max(1.);
	let page_height = page_height.unwrap_or(tree.size().height() as f32).max(1.);

	// Create a krilla PDF document
	let mut document = krilla::Document::new();

	let page_settings = PageSettings::from_wh(page_width, page_height).ok_or_else(|| "Invalid page dimensions for PDF export".to_string())?;

	let mut page = document.start_page_with(page_settings);
	let mut surface = page.surface();

	let krilla_size = krilla::geom::Size::from_wh(page_width, page_height).ok_or_else(|| "Invalid size for PDF export".to_string())?;
	surface.draw_svg(&tree, krilla_size, SvgSettings::default());

	surface.finish();
	page.finish();

	let pdf_bytes = document.finish().map_err(|e| format!("Failed to generate PDF: {e:?}"))?;
	Ok(pdf_bytes)
}
