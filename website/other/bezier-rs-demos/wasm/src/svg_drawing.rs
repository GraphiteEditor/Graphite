// SVG drawing constants
pub const SVG_OPEN_TAG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200px" height="200px">"#;
pub const SVG_CLOSE_TAG: &str = "</svg>";

// Stylistic constants
pub const BLACK: &str = "black";
pub const GRAY: &str = "gray";
pub const RED: &str = "red";

// Default attributes
pub const CURVE_ATTRIBUTES: &str = "stroke=\"black\" stroke-width=\"2\" fill=\"none\"";
pub const HANDLE_LINE_ATTRIBUTES: &str = "stroke=\"gray\" stroke-width=\"1\" fill=\"none\"";
pub const ANCHOR_ATTRIBUTES: &str = "r=\"4\" stroke=\"black\" stroke-width=\"2\" fill=\"white\"";
pub const HANDLE_ATTRIBUTES: &str = "r=\"3\" stroke=\"gray\" stroke-width=\"1.5\" fill=\"white\"";

/// Helper function to create an SVG text entitty.
pub fn draw_text(text: String, x_pos: f64, y_pos: f64, fill: &str) -> String {
	format!(r#"<text x="{x_pos}" y="{y_pos}" fill="{fill}">{text}</text>"#)
}
