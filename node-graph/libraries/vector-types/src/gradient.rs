use core_types::{Color, render_complexity::RenderComplexity};
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, serde::Serialize, serde::Deserialize, DynAny, specta::Type, node_macro::ChoiceType)]
#[widget(Radio)]
pub enum GradientType {
	#[default]
	Linear,
	Radial,
}

// TODO: Someday we could switch this to a Box[T] to avoid over-allocation
// TODO: Use linear not gamma colors
/// A list of colors associated with positions (in the range 0 to 1) along a gradient.
#[derive(Debug, Clone, PartialEq, serde::Serialize, DynAny, specta::Type)]
pub struct GradientStops {
	/// The position of this stop, a factor from 0-1 along the length of the full gradient.
	pub position: Vec<f64>,
	/// The midpoint to the right of this stop, a factor from 0-1 along the distance to the next stop. The final stop's midpoint is ignored.
	pub midpoint: Vec<f64>,
	/// The color at this stop.
	pub color: Vec<Color>,
}

// TODO: Eventually remove this migration document upgrade code
impl<'de> serde::Deserialize<'de> for GradientStops {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(serde::Deserialize)]
		struct NewFormat {
			position: Vec<f64>,
			midpoint: Vec<f64>,
			color: Vec<Color>,
		}

		#[derive(serde::Deserialize)]
		#[serde(untagged)]
		enum GradientStopsFormat {
			New(NewFormat),
			Old(Vec<(f64, Color)>),
		}

		Ok(match GradientStopsFormat::deserialize(deserializer)? {
			GradientStopsFormat::New(new) => Self {
				position: new.position,
				midpoint: new.midpoint,
				color: new.color,
			},
			GradientStopsFormat::Old(stops) => {
				let count = stops.len();
				Self {
					position: stops.iter().map(|(p, _)| *p).collect(),
					midpoint: vec![0.5; count],
					color: stops.into_iter().map(|(_, c)| c).collect(),
				}
			}
		})
	}
}

impl std::hash::Hash for GradientStops {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.position.len().hash(state);
		for i in 0..self.position.len() {
			self.position[i].to_bits().hash(state);
			self.midpoint[i].to_bits().hash(state);
			self.color[i].hash(state);
		}
	}
}

impl Default for GradientStops {
	fn default() -> Self {
		Self {
			position: vec![0., 1.],
			midpoint: vec![0.5, 0.5],
			color: vec![Color::BLACK, Color::WHITE],
		}
	}
}

impl RenderComplexity for GradientStops {
	fn render_complexity(&self) -> usize {
		1
	}
}

/// Apply the midpoint curve to a normalized parameter `t` (0 to 1) given a `midpoint` (0 to 1, where 0.5 is linear).
fn apply_midpoint(t: f64, midpoint: f64) -> f64 {
	if (midpoint - 0.5).abs() < 1e-6 {
		return t;
	}

	let midpoint = midpoint.clamp(f64::EPSILON, 1. - f64::EPSILON);

	if midpoint < 0.5 {
		let q = -1. / (1. - midpoint).log2();
		1. - (1. - t).powf(q)
	} else {
		let p = -1. / midpoint.log2();
		t.powf(p)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct GradientStop {
	pub position: f64,
	pub midpoint: f64,
	pub color: Color,
}

pub struct GradientStopsIter<'a> {
	stops: &'a GradientStops,
	index: usize,
}

impl<'a> Iterator for GradientStopsIter<'a> {
	type Item = GradientStop;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.stops.position.len() {
			return None;
		}

		let stop = GradientStop {
			position: self.stops.position[self.index],
			midpoint: self.stops.midpoint[self.index],
			color: self.stops.color[self.index],
		};
		self.index += 1;
		Some(stop)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.stops.position.len() - self.index;
		(remaining, Some(remaining))
	}
}

impl ExactSizeIterator for GradientStopsIter<'_> {}

impl<'a> IntoIterator for &'a GradientStops {
	type Item = GradientStop;
	type IntoIter = GradientStopsIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		GradientStopsIter { stops: self, index: 0 }
	}
}

impl IntoIterator for GradientStops {
	type Item = GradientStop;
	type IntoIter = std::vec::IntoIter<GradientStop>;

	fn into_iter(self) -> Self::IntoIter {
		self.position
			.into_iter()
			.zip(self.midpoint)
			.zip(self.color)
			.map(|((position, midpoint), color)| GradientStop { position, midpoint, color })
			.collect::<Vec<_>>()
			.into_iter()
	}
}

impl GradientStops {
	pub fn new(stops: impl IntoIterator<Item = GradientStop>) -> Self {
		let mut position = Vec::new();
		let mut midpoint = Vec::new();
		let mut color = Vec::new();

		for stop in stops {
			position.push(stop.position);
			midpoint.push(stop.midpoint);
			color.push(stop.color);
		}

		Self { position, midpoint, color }
	}

	pub fn len(&self) -> usize {
		self.position.len()
	}

	pub fn is_empty(&self) -> bool {
		self.position.is_empty()
	}

	pub fn iter(&self) -> GradientStopsIter<'_> {
		self.into_iter()
	}

	/// Remove a stop at the given index.
	pub fn remove(&mut self, index: usize) {
		self.position.remove(index);
		self.midpoint.remove(index);
		self.color.remove(index);
	}

	/// Remove and return the last stop's color, or `None` if empty.
	pub fn pop(&mut self) -> Option<Color> {
		self.position.pop();
		self.midpoint.pop();
		self.color.pop()
	}

	pub fn evaluate(&self, t: f64) -> Color {
		if self.position.is_empty() {
			return Color::BLACK;
		}

		if t <= self.position[0] {
			return self.color[0];
		}
		let last = self.position.len() - 1;
		if t >= self.position[last] {
			return self.color[last];
		}

		for i in 0..self.position.len() - 1 {
			let (t1, c1) = (self.position[i], self.color[i]);
			let (t2, c2) = (self.position[i + 1], self.color[i + 1]);
			if t >= t1 && t <= t2 {
				let normalized_t = (t - t1) / (t2 - t1);
				let adjusted_t = apply_midpoint(normalized_t, self.midpoint[i]);
				return c1.lerp(&c2, adjusted_t as f32);
			}
		}

		Color::BLACK
	}

	pub fn sort(&mut self) {
		let mut indices: Vec<usize> = (0..self.position.len()).collect();
		indices.sort_unstable_by(|&a, &b| self.position[a].partial_cmp(&self.position[b]).unwrap());
		self.position = indices.iter().map(|&i| self.position[i]).collect();
		self.midpoint = indices.iter().map(|&i| self.midpoint[i]).collect();
		self.color = indices.iter().map(|&i| self.color[i]).collect();
	}

	pub fn reversed(&self) -> Self {
		let position: Vec<f64> = self.position.iter().rev().map(|&p| 1. - p).collect();

		let count = self.midpoint.len();
		let midpoint = (0..count).map(|i| if i < count - 1 { 1. - self.midpoint[count - 2 - i] } else { 0.5 }).collect::<Vec<_>>();

		let color: Vec<Color> = self.color.iter().rev().cloned().collect();

		Self { position, midpoint, color }
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		Self {
			position: self.position.clone(),
			midpoint: self.midpoint.clone(),
			color: self.color.iter().map(f).collect(),
		}
	}

	/// Produce a set of linearly-interpolated color samples that approximate the gradient's midpoint curves.
	///
	/// Each sample is `(position, color, original_midpoint)` where `original_midpoint` is `Some(f64)` with the corresponding
	/// midpoint for actual gradient stops, and `None` for interpolated samples added to approximate midpoint curves.
	pub fn interpolated_samples(&self) -> Vec<(f64, Color, Option<f64>)> {
		/// Controls accuracy vs. number of samples tradeoff.
		/// 2/255 means the linear approximation will deviate by no more than 2 gradations of 8-bit color from the theoretically perfect curve with this midpoint bias.
		const THRESHOLD: f64 = 2. / 255.;

		#[allow(clippy::too_many_arguments)]
		fn subdivide(left: f64, right: f64, midpoint: f64, pos_a: f64, pos_b: f64, color_a: Color, color_b: Color, result: &mut Vec<(f64, Color, Option<f64>)>, depth: u32) {
			const MAX_DEPTH: u32 = 20;
			if depth >= MAX_DEPTH {
				return;
			}

			let mid = (left + right) / 2.;

			let y_actual = apply_midpoint(mid, midpoint);
			let y_left = apply_midpoint(left, midpoint);
			let y_right = apply_midpoint(right, midpoint);
			let y_linear = (y_left + y_right) / 2.;

			if (y_actual - y_linear).abs() > THRESHOLD {
				subdivide(left, mid, midpoint, pos_a, pos_b, color_a, color_b, result, depth + 1);

				let global_pos = pos_a + mid * (pos_b - pos_a);
				let color = color_a.lerp(&color_b, y_actual as f32);
				result.push((global_pos, color, None));

				subdivide(mid, right, midpoint, pos_a, pos_b, color_a, color_b, result, depth + 1);
			}
		}

		if self.position.is_empty() {
			return vec![];
		}

		if self.position.len() == 1 {
			return vec![(self.position[0], self.color[0], Some(self.midpoint[0]))];
		}

		let mut result = Vec::new();

		for i in 0..self.position.len() - 1 {
			let pos_a = self.position[i];
			let pos_b = self.position[i + 1];
			let color_a = self.color[i];
			let color_b = self.color[i + 1];
			let midpoint = self.midpoint[i].clamp(0.01, 0.99);
			let next_midpoint = self.midpoint[i + 1].clamp(0.01, 0.99);

			// Add the start stop (subsequent segments share the previous end stop)
			if i == 0 {
				result.push((pos_a, color_a, Some(midpoint)));
			}

			// Only subdivide if midpoint deviates from linear (0.5)
			if (midpoint - 0.5).abs() >= 1e-6 {
				subdivide(0., 1., midpoint, pos_a, pos_b, color_a, color_b, &mut result, 0);
			}

			// Add the end stop
			result.push((pos_b, color_b, Some(next_midpoint)));
		}

		// If every midpoint is 0.5 (or within epsilon), turn all midpoints to None
		if result.iter().all(|(_, _, midpoint)| matches!(midpoint, Some(m) if (m - 0.5).abs() < 1e-6)) {
			result.iter_mut().for_each(|(_, _, midpoint)| *midpoint = None);
		}

		result
	}
}

/// A gradient fill.
///
/// Contains the start and end points, along with the colors at varying points along the length.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
pub struct Gradient {
	pub stops: GradientStops,
	pub gradient_type: GradientType,
	pub start: DVec2,
	pub end: DVec2,
}

impl Default for Gradient {
	fn default() -> Self {
		Self {
			stops: GradientStops::default(),
			gradient_type: GradientType::Linear,
			start: DVec2::new(0., 0.5),
			end: DVec2::new(1., 0.5),
		}
	}
}

impl std::hash::Hash for Gradient {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.stops.len().hash(state);
		[].iter()
			.chain(self.start.to_array().iter())
			.chain(self.end.to_array().iter())
			.chain(self.stops.position.iter())
			.chain(self.stops.midpoint.iter())
			.for_each(|x| x.to_bits().hash(state));
		self.stops.color.iter().for_each(|color| color.hash(state));
		self.gradient_type.hash(state);
	}
}

impl std::fmt::Display for Gradient {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let round = |x: f64| (x * 1e3).round() / 1e3;
		let stops = self
			.stops
			.iter()
			.map(|stop| format!("[{}%: #{}]", round(stop.position * 100.), stop.color.to_rgba_hex_srgb()))
			.collect::<Vec<_>>()
			.join(", ");
		write!(f, "{} Gradient: {stops}", self.gradient_type)
	}
}

impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, gradient_type: GradientType) -> Self {
		let stops = GradientStops::new([
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: start_color.to_gamma_srgb(),
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: end_color.to_gamma_srgb(),
			},
		]);

		Self { start, end, stops, gradient_type }
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let start = self.start + (other.start - self.start) * time;
		let end = self.end + (other.end - self.end) * time;
		let stops = self.stops.iter().zip(other.stops.iter()).map(|(a, b)| {
			let position = a.position + (b.position - a.position) * time;
			let color = a.color.lerp(&b.color, time as f32);
			GradientStop { position, midpoint: 0.5, color }
		});
		let stops = GradientStops::new(stops);
		let gradient_type = if time < 0.5 { self.gradient_type } else { other.gradient_type };

		Self { start, end, stops, gradient_type }
	}

	/// Insert a stop into the gradient, the index if successful
	pub fn insert_stop(&mut self, mouse: DVec2, transform: DAffine2) -> Option<usize> {
		// Transform the start and end positions to the same coordinate space as the mouse.
		let (start, end) = (transform.transform_point2(self.start), transform.transform_point2(self.end));

		// Calculate the new position by finding the closest point on the line
		let new_position = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

		// Don't insert point past end of line
		if !(0. ..=1.).contains(&new_position) {
			return None;
		}

		// Compute the color of the inserted stop using evaluate (which respects midpoints)
		let new_color = self.stops.evaluate(new_position);

		// Compute the correct index to keep the positions in order
		let mut index = 0;
		while self.stops.len() > index && self.stops.position[index] <= new_position {
			index += 1;
		}

		// Insert the new stop
		self.stops.position.insert(index, new_position);
		self.stops.midpoint.insert(index, 0.5);
		self.stops.color.insert(index, new_color);

		Some(index)
	}
}

impl core_types::bounds::BoundingBox for GradientStops {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		core_types::bounds::RenderBoundingBox::Infinite
	}
}
