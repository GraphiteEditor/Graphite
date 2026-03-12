use krilla::page::PageSettings;
use krilla_svg::{SurfaceExt, SvgSettings};

/// Draw one SVG onto an already-open krilla `Document` as a new page.

fn add_svg_page(document: &mut krilla::Document, svg: &str, page_width: f32, page_height: f32) -> Result<(), String> {
	let options = usvg_045::Options::default();
	let tree = usvg_045::Tree::from_str(svg, &options).map_err(|e| format!("Failed to parse SVG for PDF: {e}"))?;

	let page_width = page_width.max(1.);
	let page_height = page_height.max(1.);

	let page_settings = PageSettings::from_wh(page_width, page_height).ok_or_else(|| "Invalid page dimensions for PDF export".to_string())?;

	let mut page = document.start_page_with(page_settings);
	let mut surface = page.surface();

	let krilla_size = krilla::geom::Size::from_wh(page_width, page_height).ok_or_else(|| "Invalid size for PDF export".to_string())?;
	surface.draw_svg(&tree, krilla_size, SvgSettings::default());

	surface.finish();
	page.finish();
	Ok(())
}

/// Convert a single SVG string to a one-page PDF.
pub fn svg_to_pdf(svg: &str, page_width: Option<f32>, page_height: Option<f32>) -> Result<Vec<u8>, String> {
	let options = usvg_045::Options::default();
	let tree = usvg_045::Tree::from_str(svg, &options).map_err(|e| format!("Failed to parse SVG for PDF conversion: {e}"))?;

	let width = page_width.unwrap_or(tree.size().width() as f32);
	let height = page_height.unwrap_or(tree.size().height() as f32);

	let mut document = krilla::Document::new();
	add_svg_page(&mut document, svg, width, height)?;
	document.finish().map_err(|e| format!("Failed to generate PDF: {e:?}"))
}

/// Convert multiple SVGs into a single multi-page PDF, one page per SVG.

pub fn svg_pages_to_pdf(pages: &[(String, f32, f32)]) -> Result<Vec<u8>, String> {
	if pages.is_empty() {
		return Err("No pages provided for multi-page PDF export".to_string());
	}

	let mut document = krilla::Document::new();
	for (svg, w, h) in pages {
		add_svg_page(&mut document, svg, *w, *h)?;
	}
	document.finish().map_err(|e| format!("Failed to generate multi-page PDF: {e:?}"))
}
