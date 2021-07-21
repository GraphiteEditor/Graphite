use glam::DVec2;

pub fn merge_bounding_boxes([a_min, a_max]: [DVec2; 2], [b_min, b_max]: [DVec2; 2]) -> [DVec2; 2] {
	let min_x = a_min.x.min(b_min.x);
	let min_y = a_min.y.min(b_min.y);
	let max_x = a_max.x.max(b_max.x);
	let max_y = a_max.y.max(b_max.y);
	[DVec2::new(min_x, min_y), DVec2::new(max_x, max_y)]
}
