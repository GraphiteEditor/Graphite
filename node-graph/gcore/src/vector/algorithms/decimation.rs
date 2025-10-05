use glam::DVec2;

/// Implements the Ramer-Douglas-Peucker algorithm to find indices of points to keep.
pub fn ramer_douglas_peucker(points: &[DVec2], epsilon: f64) -> Vec<usize> {
	if points.len() <= 2 {
		return (0..points.len()).collect();
	}

	let mut kept_indices = Vec::new();
	rdp_recursive(points, 0, points.len() - 1, epsilon, &mut kept_indices);
	kept_indices.sort_unstable();
	kept_indices.dedup();
	kept_indices
}

pub fn rdp_recursive(points: &[DVec2], start_idx: usize, end_idx: usize, epsilon: f64, kept_indices: &mut Vec<usize>) {
	if start_idx >= end_idx {
		return;
	}

	let mut dmax = 0.0;
	let mut index = start_idx;

	// Find the point with maximum perpendicular distance from the line segment
	for i in (start_idx + 1)..end_idx {
		let d = perpendicular_distance(points[i], points[start_idx], points[end_idx]);
		if d > dmax {
			index = i;
			dmax = d;
		}
	}

	// If max distance is greater than epsilon, recursively simplify
	if dmax > epsilon {
		rdp_recursive(points, start_idx, index, epsilon, kept_indices);
		rdp_recursive(points, index, end_idx, epsilon, kept_indices);
	} else {
		// Keep only the endpoints
		kept_indices.push(start_idx);
		kept_indices.push(end_idx);
	}
}

/// Calculates the perpendicular distance from a point to a line defined by two endpoints.
pub fn perpendicular_distance(point: DVec2, line_start: DVec2, line_end: DVec2) -> f64 {
	let dx = line_end.x - line_start.x;
	let dy = line_end.y - line_start.y;

	let line_length_squared = dx * dx + dy * dy;

	// If the line segment is actually a point, return the distance to that point
	if line_length_squared == 0.0 {
		return point.distance(line_start);
	}

	// Calculate perpendicular distance using the cross product formula:
	// distance = |dy·px - dx·py + x2·y1 - y2·x1| / √(dx² + dy²)
	let numerator = (dy * point.x - dx * point.y + line_end.x * line_start.y - line_end.y * line_start.x).abs();
	let denominator = line_length_squared.sqrt();

	numerator / denominator
}
