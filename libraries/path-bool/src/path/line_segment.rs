use glam::DVec2;

pub type LineSegment = [DVec2; 2];

const COLLINEAR_EPS: f64 = f64::EPSILON * 64.;

#[inline(never)]
pub fn line_segment_intersection([p1, p2]: LineSegment, [p3, p4]: LineSegment, eps: f64) -> Option<(f64, f64)> {
	// https://en.wikipedia.org/wiki/Intersection_(geometry)#Two_line_segments

	let a = p2 - p1;
	let b = p3 - p4;
	let c = p3 - p1;

	let denom = a.x * b.y - a.y * b.x;

	if denom.abs() < COLLINEAR_EPS {
		return None;
	}

	let s = (c.x * b.y - c.y * b.x) / denom;
	let t = (a.x * c.y - a.y * c.x) / denom;

	if (-eps..=1. + eps).contains(&s) && (-eps..=1. + eps).contains(&t) { Some((s, t)) } else { None }
}

pub fn line_segments_intersect(seg1: LineSegment, seg2: LineSegment, eps: f64) -> bool {
	line_segment_intersection(seg1, seg2, eps).is_some()
}
