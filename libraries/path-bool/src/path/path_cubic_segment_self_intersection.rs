use crate::path_segment::PathSegment;

const EPS: f64 = 1e-12;

pub fn path_cubic_segment_self_intersection(seg: &PathSegment) -> Option<[f64; 2]> {
	// https://math.stackexchange.com/questions/3931865/self-intersection-of-a-cubic-bezier-interpretation-of-the-solution

	if let PathSegment::Cubic(p1, p2, p3, p4) = seg {
		let ax = -p1.x + 3. * p2.x - 3. * p3.x + p4.x;
		let ay = -p1.y + 3. * p2.y - 3. * p3.y + p4.y;
		let bx = 3. * p1.x - 6. * p2.x + 3. * p3.x;
		let by = 3. * p1.y - 6. * p2.y + 3. * p3.y;
		let cx = -3. * p1.x + 3. * p2.x;
		let cy = -3. * p1.y + 3. * p2.y;

		let m = ay * bx - ax * by;
		let n = ax * cy - ay * cx;

		let k = (-3. * ax * ax * cy * cy + 6. * ax * ay * cx * cy + 4. * ax * bx * by * cy - 4. * ax * by * by * cx - 3. * ay * ay * cx * cx - 4. * ay * bx * bx * cy + 4. * ay * bx * by * cx)
			/ (ax * ax * by * by - 2. * ax * ay * bx * by + ay * ay * bx * bx);

		if k < 0. {
			return None;
		}

		let t1 = (n / m + k.sqrt()) / 2.;
		let t2 = (n / m - k.sqrt()) / 2.;

		if (EPS..=1. - EPS).contains(&t1) && (EPS..=1. - EPS).contains(&t2) {
			let mut result = [t1, t2];
			result.sort_by(|a, b| a.partial_cmp(b).unwrap());
			Some(result)
		} else {
			None
		}
	} else {
		None
	}
}
