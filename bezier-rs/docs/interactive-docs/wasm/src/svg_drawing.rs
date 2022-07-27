// SVG drawing constants
pub const SVG_OPEN_TAG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200px" height="200px">"#;
pub const SVG_CLOSE_TAG: &str = "</svg>";

// Sylistic constants
pub const BLACK: &str = "black";

/// Helper function to create an SVG text entitty.
pub fn draw_text(text: String, x_pos: f64, y_pos: f64, fill: &str) -> String {
	format!(r#"<text x="{x_pos}" y="{y_pos}" fill="{fill}">{text}</text>"#)
}
