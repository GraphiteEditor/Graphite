use core_types::color::Color;
use core_types::list::Item;
use glam::DVec2;
use raster_types::{Bitmap, BitmapMut, CPU, Image, Raster};

/// Samples a pixel from an image at continuous coordinates `(x, y)` using bilinear interpolation.
pub fn sample_bilinear(image: &Image<Color>, x: f64, y: f64) -> Color {
	if image.width == 0 || image.height == 0 {
		return Color::TRANSPARENT;
	}

	// Shift from pixel center coordinate to continuous index
	let x = x - 0.5;
	let y = y - 0.5;

	let x0 = x.floor() as i64;
	let y0 = y.floor() as i64;
	let x1 = x0 + 1;
	let y1 = y0 + 1;

	let fx = (x - x0 as f64) as f32;
	let fy = (y - y0 as f64) as f32;

	let get = |ix: i64, iy: i64| -> Color {
		if ix >= 0 && iy >= 0 && (ix as u32) < image.width && (iy as u32) < image.height {
			image.get_pixel(ix as u32, iy as u32).unwrap_or(Color::TRANSPARENT)
		} else {
			Color::TRANSPARENT
		}
	};

	let c00 = get(x0, y0);
	let c10 = get(x1, y0);
	let c01 = get(x0, y1);
	let c11 = get(x1, y1);

	// Bilinear blend across 4 neighbors
	let top_r = c00.r() * (1. - fx) + c10.r() * fx;
	let top_g = c00.g() * (1. - fx) + c10.g() * fx;
	let top_b = c00.b() * (1. - fx) + c10.b() * fx;
	let top_a = c00.a() * (1. - fx) + c10.a() * fx;

	let bot_r = c01.r() * (1. - fx) + c11.r() * fx;
	let bot_g = c01.g() * (1. - fx) + c11.g() * fx;
	let bot_b = c01.b() * (1. - fx) + c11.b() * fx;
	let bot_a = c01.a() * (1. - fx) + c11.a() * fx;

	Color::from_rgbaf32_unchecked(
		top_r * (1. - fy) + bot_r * fy,
		top_g * (1. - fy) + bot_g * fy,
		top_b * (1. - fy) + bot_b * fy,
		top_a * (1. - fy) + bot_a * fy,
	)
}

/// Computes the barycentric coordinates `(alpha, beta, gamma)` of point `p` relative to triangle `(a, b, c)`.
/// Returns `None` if the triangle is degenerate (colinear points).
#[inline]
pub fn barycentric_coordinates(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> Option<(f64, f64, f64)> {
	let v0 = b - a;
	let v1 = c - a;
	let v2 = p - a;

	let denom = v0.x * v1.y - v1.x * v0.y;
	if denom.abs() < 1e-12 {
		return None;
	}

	let inv_denom = 1.0 / denom;
	let beta = (v2.x * v1.y - v1.x * v2.y) * inv_denom;
	let gamma = (v0.x * v2.y - v2.x * v0.y) * inv_denom;
	let alpha = 1.0 - beta - gamma;

	Some((alpha, beta, gamma))
}

/// Warps a single triangle region from `src` to `dst`.
///
/// * `src`: Source image in rest pose.
/// * `dst`: Target image in deformed pose.
/// * `rest`: Triangle vertices `[A0, B0, C0]` in source image coordinates.
/// * `deformed`: Triangle vertices `[A, B, C]` in target image coordinates.
pub fn warp_triangle(src: &Image<Color>, dst: &mut Image<Color>, rest: [DVec2; 3], deformed: [DVec2; 3]) {
	if dst.width == 0 || dst.height == 0 {
		return;
	}

	let min_x = deformed.iter().map(|p| p.x).fold(f64::INFINITY, f64::min).floor().max(0.0) as u32;
	let max_x = deformed.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max).ceil().min((dst.width - 1) as f64) as u32;
	let min_y = deformed.iter().map(|p| p.y).fold(f64::INFINITY, f64::min).floor().max(0.0) as u32;
	let max_y = deformed.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max).ceil().min((dst.height - 1) as f64) as u32;

	if min_x > max_x || min_y > max_y {
		return;
	}

	let epsilon = -1e-4;

	for y in min_y..=max_y {
		for x in min_x..=max_x {
			let p = DVec2::new(x as f64 + 0.5, y as f64 + 0.5);
			let Some((alpha, beta, gamma)) = barycentric_coordinates(p, deformed[0], deformed[1], deformed[2]) else {
				continue;
			};
			if alpha >= epsilon && beta >= epsilon && gamma >= epsilon {
				let p0 = alpha * rest[0] + beta * rest[1] + gamma * rest[2];
				let color = sample_bilinear(src, p0.x, p0.y);
				if let Some(pixel) = dst.get_pixel_mut(x, y) {
					*pixel = color;
				}
			}
		}
	}
}

/// Triangulates points into a list of triangles, each represented as a triple of indices.
pub fn delaunay_triangles(sites: &[DVec2]) -> Vec<[usize; 3]> {
	if sites.len() < 3 {
		return Vec::new();
	}
	let points: Vec<delaunator::Point> = sites.iter().map(|p| delaunator::Point { x: p.x, y: p.y }).collect();
	let triangulation = delaunator::triangulate(&points);
	triangulation.triangles.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect()
}

/// Warps a mesh of triangles from `src` to `dst`.
pub fn warp_mesh(src: &Image<Color>, dst: &mut Image<Color>, rest_vertices: &[DVec2], deformed_vertices: &[DVec2], triangles: &[[usize; 3]]) {
	for &[a, b, c] in triangles {
		if a >= rest_vertices.len() || b >= rest_vertices.len() || c >= rest_vertices.len() {
			continue;
		}
		if a >= deformed_vertices.len() || b >= deformed_vertices.len() || c >= deformed_vertices.len() {
			continue;
		}

		let rest = [rest_vertices[a], rest_vertices[b], rest_vertices[c]];
		let deformed = [deformed_vertices[a], deformed_vertices[b], deformed_vertices[c]];
		warp_triangle(src, dst, rest, deformed);
	}
}

/// Warps a raster image using a 2D mesh defined by rest and deformed control points.
#[node_macro::node(category("Raster: Warp"), icon("NodeTransform"))]
async fn mesh_warp(
	_: impl core_types::context::Ctx,
	/// The image to be warped.
	image_frame: Item<Raster<CPU>>,
	/// The original undeformed control points (rest pose).
	rest_points: Item<Vec<DVec2>>,
	/// The deformed control points (matching indices with rest points).
	deformed_points: Item<Vec<DVec2>>,
) -> Item<Raster<CPU>> {
	let (image, attributes) = image_frame.into_parts();
	let (rest, deformed) = (rest_points.element(), deformed_points.element());

	if rest.len() < 3 || deformed.len() < 3 || rest.len() != deformed.len() {
		return Item::from_parts(image, attributes);
	}

	let triangles = delaunay_triangles(rest);
	if triangles.is_empty() {
		return Item::from_parts(image, attributes);
	}

	let src = image.into_data();
	let mut dst = Image::new(src.width, src.height, Color::TRANSPARENT);

	warp_mesh(&src, &mut dst, rest, deformed, &triangles);

	Item::from_parts(Raster::new_cpu(dst), attributes)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_barycentric_inside_triangle() {
		let a = DVec2::new(0.0, 0.0);
		let b = DVec2::new(10.0, 0.0);
		let c = DVec2::new(0.0, 10.0);

		// Centroid: (10/3, 10/3)
		let centroid = DVec2::new(10.0 / 3.0, 10.0 / 3.0);
		let coords = barycentric_coordinates(centroid, a, b, c).expect("triangle should be non-degenerate");
		assert!((coords.0 - 1.0 / 3.0).abs() < 1e-6);
		assert!((coords.1 - 1.0 / 3.0).abs() < 1e-6);
		assert!((coords.2 - 1.0 / 3.0).abs() < 1e-6);

		// Outside point
		let outside = DVec2::new(15.0, 15.0);
		let (alpha, beta, gamma) = barycentric_coordinates(outside, a, b, c).unwrap();
		assert!(alpha < 0.0 || beta < 0.0 || gamma < 0.0);
	}

	#[test]
	fn test_identity_warp() {
		let width = 20;
		let height = 20;
		let red = Color::from_rgbaf32_unchecked(1.0, 0.0, 0.0, 1.0);
		let mut src = Image::new(width, height, Color::TRANSPARENT);

		// Fill a center 10x10 square with red
		for y in 5..15 {
			for x in 5..15 {
				*src.get_pixel_mut(x, y).unwrap() = red;
			}
		}

		let mut dst = Image::new(width, height, Color::TRANSPARENT);

		// Identity mesh covering (0,0) to (20,20) as 2 triangles
		let v0 = DVec2::new(0.0, 0.0);
		let v1 = DVec2::new(20.0, 0.0);
		let v2 = DVec2::new(20.0, 20.0);
		let v3 = DVec2::new(0.0, 20.0);

		let vertices = vec![v0, v1, v2, v3];
		let triangles = vec![[0, 1, 2], [0, 2, 3]];

		warp_mesh(&src, &mut dst, &vertices, &vertices, &triangles);

		// Check that the center pixel (10, 10) in dst is red
		let center = dst.get_pixel(10, 10).unwrap();
		assert!((center.r() - 1.0).abs() < 0.05);
		assert!(center.a() > 0.95);
	}

	#[test]
	fn test_translation_warp() {
		let width = 30;
		let height = 30;
		let blue = Color::from_rgbaf32_unchecked(0.0, 0.0, 1.0, 1.0);
		let mut src = Image::new(width, height, Color::TRANSPARENT);

		// Set point at interior (3, 3)
		*src.get_pixel_mut(3, 3).unwrap() = blue;

		let mut dst = Image::new(width, height, Color::TRANSPARENT);

		let rest = [DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(0.0, 10.0)];
		// Translated by +10 in x and y
		let deformed = [DVec2::new(10.0, 10.0), DVec2::new(20.0, 10.0), DVec2::new(10.0, 20.0)];

		warp_triangle(&src, &mut dst, rest, deformed);

		// The pixel at (3, 3) in src should now appear at (13, 13) in dst
		let mapped = dst.get_pixel(13, 13).unwrap();
		assert!(mapped.b() > 0.5);
	}
}
