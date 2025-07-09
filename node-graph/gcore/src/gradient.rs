use crate::Color;
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, DynAny, specta::Type)]
pub struct GradientStops(pub Vec<(f64, Color)>);

impl std::hash::Hash for GradientStops {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.len().hash(state);
		self.0.iter().for_each(|(position, color)| {
			position.to_bits().hash(state);
			color.hash(state);
		});
	}
}

impl Default for GradientStops {
	fn default() -> Self {
		Self(vec![(0., Color::BLACK), (1., Color::WHITE)])
	}
}

impl IntoIterator for GradientStops {
	type Item = (f64, Color);
	type IntoIter = std::vec::IntoIter<(f64, Color)>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

impl<'a> IntoIterator for &'a GradientStops {
	type Item = &'a (f64, Color);
	type IntoIter = std::slice::Iter<'a, (f64, Color)>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.iter()
	}
}

impl std::ops::Index<usize> for GradientStops {
	type Output = (f64, Color);

	fn index(&self, index: usize) -> &Self::Output {
		&self.0[index]
	}
}

impl std::ops::Deref for GradientStops {
	type Target = Vec<(f64, Color)>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for GradientStops {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl GradientStops {
	pub fn new(stops: Vec<(f64, Color)>) -> Self {
		let mut stops = Self(stops);
		stops.sort();
		stops
	}

	pub fn evaluate(&self, t: f64) -> Color {
		if self.0.is_empty() {
			return Color::BLACK;
		}

		if t <= self.0[0].0 {
			return self.0[0].1;
		}
		if t >= self.0[self.0.len() - 1].0 {
			return self.0[self.0.len() - 1].1;
		}

		for i in 0..self.0.len() - 1 {
			let (t1, c1) = self.0[i];
			let (t2, c2) = self.0[i + 1];
			if t >= t1 && t <= t2 {
				let normalized_t = (t - t1) / (t2 - t1);
				return c1.lerp(&c2, normalized_t as f32);
			}
		}

		Color::BLACK
	}

	pub fn sort(&mut self) {
		self.0.sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
	}

	pub fn reversed(&self) -> Self {
		Self(self.0.iter().rev().map(|(position, color)| (1. - position, *color)).collect())
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		Self(self.0.iter().map(|(position, color)| (*position, f(color))).collect())
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
	pub transform: DAffine2,
}

impl Default for Gradient {
	fn default() -> Self {
		Self {
			stops: GradientStops::default(),
			gradient_type: GradientType::Linear,
			start: DVec2::new(0., 0.5),
			end: DVec2::new(1., 0.5),
			transform: DAffine2::IDENTITY,
		}
	}
}

impl std::hash::Hash for Gradient {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.stops.0.len().hash(state);
		[].iter()
			.chain(self.start.to_array().iter())
			.chain(self.end.to_array().iter())
			.chain(self.transform.to_cols_array().iter())
			.chain(self.stops.0.iter().map(|(position, _)| position))
			.for_each(|x| x.to_bits().hash(state));
		self.stops.0.iter().for_each(|(_, color)| color.hash(state));
		self.gradient_type.hash(state);
	}
}

impl std::fmt::Display for Gradient {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let round = |x: f64| (x * 1e3).round() / 1e3;
		let stops = self
			.stops
			.0
			.iter()
			.map(|(position, color)| format!("[{}%: #{}]", round(position * 100.), color.to_rgba_hex_srgb()))
			.collect::<Vec<_>>()
			.join(", ");
		write!(f, "{} Gradient: {stops}", self.gradient_type)
	}
}

impl Gradient {
	/// Constructs a new gradient with the colors at 0 and 1 specified.
	pub fn new(start: DVec2, start_color: Color, end: DVec2, end_color: Color, transform: DAffine2, gradient_type: GradientType) -> Self {
		Gradient {
			start,
			end,
			stops: GradientStops::new(vec![(0., start_color.to_gamma_srgb()), (1., end_color.to_gamma_srgb())]),
			transform,
			gradient_type,
		}
	}

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let start = self.start + (other.start - self.start) * time;
		let end = self.end + (other.end - self.end) * time;
		let transform = self.transform;
		let stops = self
			.stops
			.0
			.iter()
			.zip(other.stops.0.iter())
			.map(|((a_pos, a_color), (b_pos, b_color))| {
				let position = a_pos + (b_pos - a_pos) * time;
				let color = a_color.lerp(b_color, time as f32);
				(position, color)
			})
			.collect::<Vec<_>>();
		let stops = GradientStops::new(stops);
		let gradient_type = if time < 0.5 { self.gradient_type } else { other.gradient_type };

		Self {
			start,
			end,
			transform,
			stops,
			gradient_type,
		}
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

		// Compute the color of the inserted stop
		let get_color = |index: usize, time: f64| match (self.stops.0[index].1, self.stops.0.get(index + 1).map(|(_, c)| *c)) {
			// Lerp between the nearest colors if applicable
			(a, Some(b)) => a.lerp(
				&b,
				((time - self.stops.0[index].0) / self.stops.0.get(index + 1).map(|end| end.0 - self.stops.0[index].0).unwrap_or_default()) as f32,
			),
			// Use the start or the end color if applicable
			(v, _) => v,
		};

		// Compute the correct index to keep the positions in order
		let mut index = 0;
		while self.stops.0.len() > index && self.stops.0[index].0 <= new_position {
			index += 1;
		}

		let new_color = get_color(index - 1, new_position);

		// Insert the new stop
		self.stops.0.insert(index, (new_position, new_color));

		Some(index)
	}
}
