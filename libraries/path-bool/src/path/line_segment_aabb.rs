use crate::aabb::Aabb;
use crate::line_segment::LineSegment;

const INSIDE: u8 = 0;
const LEFT: u8 = 1;
const RIGHT: u8 = 1 << 1;
const BOTTOM: u8 = 1 << 2;
const TOP: u8 = 1 << 3;

fn out_code(x: f64, y: f64, bounding_box: &Aabb) -> u8 {
	let mut code = INSIDE;

	if x < bounding_box.left {
		code |= LEFT;
	} else if x > bounding_box.right {
		code |= RIGHT;
	}

	if y < bounding_box.top {
		code |= BOTTOM;
	} else if y > bounding_box.bottom {
		code |= TOP;
	}

	code
}

pub(crate) fn line_segment_aabb_intersect(seg: LineSegment, bounding_box: &Aabb) -> bool {
	let [mut p0, mut p1] = seg;

	let mut outcode0 = out_code(p0.x, p0.y, bounding_box);
	let mut outcode1 = out_code(p1.x, p1.y, bounding_box);

	loop {
		if (outcode0 | outcode1) == 0 {
			// bitwise OR is 0: both points inside window; trivially accept and exit loop
			return true;
		} else if (outcode0 & outcode1) != 0 {
			// bitwise AND is not 0: both points share an outside zone (LEFT, RIGHT, TOP,
			// or BOTTOM), so both must be outside window; exit loop (accept is false)
			return false;
		} else {
			// failed both tests, so calculate the line segment to clip
			// from an outside point to an intersection with clip edge
			let mut x = 0.;
			let mut y = 0.;

			// At least one endpoint is outside the clip rectangle; pick it.
			let outcode_out = if outcode1 > outcode0 { outcode1 } else { outcode0 };

			// Now find the intersection point;
			// use formulas:
			//   slope = (y1 - y0) / (x1 - x0)
			//   x = x0 + (1 / slope) * (ym - y0), where ym is ymin or ymax
			//   y = y0 + slope * (xm - x0), where xm is xmin or xmax
			// No need to worry about divide-by-zero because, in each case, the
			// outcode bit being tested guarantees the denominator is non-zero
			if (outcode_out & TOP) != 0 {
				// point is above the clip window
				x = p0.x + (p1.x - p0.x) * (bounding_box.bottom - p0.y) / (p1.y - p0.y);
				y = bounding_box.bottom;
			} else if (outcode_out & BOTTOM) != 0 {
				// point is below the clip window
				x = p0.x + (p1.x - p0.x) * (bounding_box.top - p0.y) / (p1.y - p0.y);
				y = bounding_box.top;
			} else if (outcode_out & RIGHT) != 0 {
				// point is to the right of clip window
				y = p0.y + (p1.y - p0.y) * (bounding_box.right - p0.x) / (p1.x - p0.x);
				x = bounding_box.right;
			} else if (outcode_out & LEFT) != 0 {
				// point is to the left of clip window
				y = p0.y + (p1.y - p0.y) * (bounding_box.left - p0.x) / (p1.x - p0.x);
				x = bounding_box.left;
			}

			// Now we move outside point to intersection point to clip
			// and get ready for next pass.
			if outcode_out == outcode0 {
				p0.x = x;
				p0.y = y;
				outcode0 = out_code(p0.x, p0.y, bounding_box);
			} else {
				p1.x = x;
				p1.y = y;
				outcode1 = out_code(p1.x, p1.y, bounding_box);
			}
		}
	}
}
