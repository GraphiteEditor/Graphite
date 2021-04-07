pub enum Operation {
	AddCircle { cx: f64, cy: f64, r: f64 },
	AddRect { x0: f64, y0: f64, x1: f64, y1: f64 },
}
