// SVG drawing constants
pub const SVG_OPEN_TAG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200px" height="200px">"#;
pub const SVG_CLOSE_TAG: &str = "</svg>";

// Stylistic constants
pub const BLACK: &str = "black";
pub const WHITE: &str = "white";
pub const GRAY: &str = "gray";
pub const RED: &str = "red";

// Default attributes
pub const CURVE_ATTRIBUTES: &str = "stroke=\"black\" stroke-width=\"2\" fill=\"none\"";
pub const HANDLE_LINE_ATTRIBUTES: &str = "stroke=\"gray\" stroke-width=\"1\" fill=\"none\"";
pub const ANCHOR_ATTRIBUTES: &str = "r=\"4\" stroke=\"black\" stroke-width=\"2\" fill=\"white\"";
pub const HANDLE_ATTRIBUTES: &str = "r=\"3\" stroke=\"gray\" stroke-width=\"1.5\" fill=\"white\"";

/// Helper function to create an SVG text entity.
pub fn draw_text(text: String, x_pos: f64, y_pos: f64, fill: &str) -> String {
	format!(r#"<text x="{x_pos}" y="{y_pos}" fill="{fill}">{text}</text>"#)
}

/// Helper function to create an SVG circle entity.
pub fn draw_circle(x_pos: f64, y_pos: f64, radius: f64, stroke: &str, stroke_width: f64, fill: &str) -> String {
	format!(r#"<circle cx="{x_pos}" cy="{y_pos}" r="{radius}" stroke="{stroke}" stroke-width="{stroke_width}" fill="{fill}"/>"#)
}

/// Helper function to create an SVG circle entity.
pub fn draw_line(start_x: f64, start_y: f64, end_x: f64, end_y: f64, stroke: &str, stroke_width: f64) -> String {
	format!(r#"<line x1="{start_x}" y1="{start_y}" x2="{end_x}" y2="{end_y}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#)
}
