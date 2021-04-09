pub enum Operation {
	AddCircle { path: String, cx: f64, cy: f64, r: f64 },
	AddRect { path: String, x0: f64, y0: f64, x1: f64, y1: f64 },
	DeleteElement { path: String },
	AddFolder { path: String },
}
