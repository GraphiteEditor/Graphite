use core_types::Color;
use core_types::color::SRGBA8;
use core_types::list::{ATTR_MIDPOINT, ATTR_POSITION, Item, List};
use core_types::render_complexity::RenderComplexity;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientType {
	#[default]
	Linear,
	Radial,
}

/// A gradient's stops: a list of colors (linear, unassociated alpha) whose optional `position` and `midpoint`
/// attributes place each stop along the 0 to 1 range. Stops lacking the `position` attribute distribute evenly,
/// and stops lacking the `midpoint` attribute interpolate linearly (`0.5`).
#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash, DynAny)]
pub struct Gradient(List<Color>);

/// A gradient's per-stop parallel arrays, generic over color format: `GradientStops<Color>` nests inside the
/// [`GradientRamp`] exchange struct, while `GradientStops<SRGBA8>` is the JS-boundary shape used by the color picker UI.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, PartialEq, Default, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientStops<C> {
	pub color: Vec<C>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub position: Option<Vec<f64>>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub midpoint: Option<Vec<f64>>,
}

unsafe impl<C: dyn_any::StaticTypeSized> dyn_any::StaticType for GradientStops<C> {
	type Static = GradientStops<C::Static>;
}

impl From<&Gradient> for GradientStops<Color> {
	fn from(gradient: &Gradient) -> Self {
		Self {
			position: gradient.position_attribute(),
			midpoint: gradient.midpoint_attribute(),
			color: gradient.0.iter_element_values().copied().collect(),
		}
	}
}

impl From<&Gradient> for GradientStops<SRGBA8> {
	fn from(gradient: &Gradient) -> Self {
		Self {
			position: gradient.position_attribute(),
			midpoint: gradient.midpoint_attribute(),
			color: gradient.0.iter_element_values().map(|&color| SRGBA8::from(color)).collect(),
		}
	}
}

// The document path: faithful (no elision) so serialization stays a bijection under round-trip checks
impl From<GradientStops<Color>> for Gradient {
	fn from(stops: GradientStops<Color>) -> Self {
		let mut gradient = Gradient::from(stops.color);
		if let Some(position) = &stops.position {
			gradient.set_positions(position);
		}
		if let Some(midpoint) = &stops.midpoint {
			gradient.set_midpoints(midpoint);
		}
		gradient
	}
}

// Color picker round-trip: attributes that merely restate the defaults are elided to keep the canonical absence-as-default form
impl From<&GradientStops<SRGBA8>> for Gradient {
	fn from(stops: &GradientStops<SRGBA8>) -> Self {
		let mut gradient = Gradient::from(stops.color.iter().map(|&color| Color::from(color)).collect::<Vec<_>>());
		if let Some(position) = &stops.position {
			gradient.set_positions(position);
		}
		if let Some(midpoint) = &stops.midpoint {
			gradient.set_midpoints(midpoint);
		}
		gradient.elide_default_attributes();
		gradient
	}
}

impl GradientStops<SRGBA8> {
	/// CSS `linear-gradient(...)` string. Stops are emitted as `#rrggbbaa` hex (already gamma-encoded bytes).
	pub fn to_css_linear_gradient(&self) -> String {
		Gradient::from(self).to_css_linear_gradient()
	}
}

/// The serialized exchange form of a gradient: its stops, nested so that whole-ramp settings
/// like spread method can join as sibling fields opted in from their defaults.
#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientRamp<C = Color> {
	pub stops: GradientStops<C>,
}

unsafe impl<C: dyn_any::StaticTypeSized> dyn_any::StaticType for GradientRamp<C> {
	type Static = GradientRamp<C::Static>;
}

impl<C> From<GradientStops<C>> for GradientRamp<C> {
	fn from(stops: GradientStops<C>) -> Self {
		Self { stops }
	}
}

impl From<&Gradient> for GradientRamp {
	fn from(gradient: &Gradient) -> Self {
		Self { stops: gradient.into() }
	}
}

impl From<Gradient> for GradientRamp {
	fn from(gradient: Gradient) -> Self {
		Self::from(&gradient)
	}
}

impl From<GradientRamp> for Gradient {
	fn from(ramp: GradientRamp) -> Self {
		Gradient::from(ramp.stops)
	}
}

impl From<&GradientRamp> for Gradient {
	fn from(ramp: &GradientRamp) -> Self {
		Gradient::from(ramp.stops.clone())
	}
}

impl From<&GradientRamp> for GradientStops<SRGBA8> {
	fn from(ramp: &GradientRamp) -> Self {
		Self {
			position: ramp.stops.position.clone(),
			midpoint: ramp.stops.midpoint.clone(),
			color: ramp.stops.color.iter().map(|&color| SRGBA8::from(color)).collect(),
		}
	}
}

// Color picker round-trip: routes through the runtime type so default-restating attributes elide
impl From<&GradientStops<SRGBA8>> for GradientRamp {
	fn from(stops: &GradientStops<SRGBA8>) -> Self {
		Self::from(Gradient::from(stops))
	}
}

impl GradientRamp {
	pub fn black_to_white() -> Self {
		Self::from(Gradient::black_to_white())
	}
}

impl From<List<Color>> for Gradient {
	fn from(colors: List<Color>) -> Self {
		Self(colors)
	}
}

impl From<Vec<Color>> for Gradient {
	fn from(colors: Vec<Color>) -> Self {
		Self(colors.into_iter().map(Item::new_from_element).collect())
	}
}

impl RenderComplexity for Gradient {
	fn render_complexity(&self) -> usize {
		1
	}
}

/// The effective midpoint domain shared by sampling and rendering: NaN reads as the linear default, and extremes are bounded to `0.01..=0.99` so curves stay finite and cheap to subdivide.
fn sanitized_midpoint(midpoint: f64) -> f64 {
	if midpoint.is_nan() { 0.5 } else { midpoint.clamp(0.01, 0.99) }
}

/// Apply the midpoint curve to a normalized parameter `t` (0 to 1) given a `midpoint` (0 to 1, where 0.5 is linear).
fn apply_midpoint(t: f64, midpoint: f64) -> f64 {
	let midpoint = sanitized_midpoint(midpoint);
	if (midpoint - 0.5).abs() < 1e-6 {
		return t;
	}

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
	stops: &'a Gradient,
	index: usize,
}

impl Iterator for GradientStopsIter<'_> {
	type Item = GradientStop;

	fn next(&mut self) -> Option<Self::Item> {
		let stop = GradientStop {
			position: self.stops.position(self.index),
			midpoint: self.stops.midpoint(self.index),
			color: self.stops.color(self.index)?,
		};
		self.index += 1;
		Some(stop)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.stops.len().saturating_sub(self.index);
		(remaining, Some(remaining))
	}
}

impl ExactSizeIterator for GradientStopsIter<'_> {}

impl<'a> IntoIterator for &'a Gradient {
	type Item = GradientStop;
	type IntoIter = GradientStopsIter<'a>;

	fn into_iter(self) -> Self::IntoIter {
		GradientStopsIter { stops: self, index: 0 }
	}
}

impl IntoIterator for Gradient {
	type Item = GradientStop;
	type IntoIter = std::vec::IntoIter<GradientStop>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter().collect::<Vec<_>>().into_iter()
	}
}

/// The fallback position of the gradient stop at `index` when no `position` attribute exists, where all `count` stops are spaced evenly from 0 to 1.
fn even_position(index: usize, count: usize) -> f64 {
	if count <= 1 { 0. } else { index as f64 / (count - 1) as f64 }
}

impl Gradient {
	pub fn new(stops: impl IntoIterator<Item = GradientStop>) -> Self {
		let stops: Vec<GradientStop> = stops.into_iter().collect();
		let mut list: List<Color> = stops.iter().map(|stop| Item::new_from_element(stop.color)).collect();

		for (index, stop) in stops.iter().enumerate() {
			list.set_attribute(ATTR_POSITION, index, stop.position);
			list.set_attribute(ATTR_MIDPOINT, index, stop.midpoint);
		}

		Self(list)
	}

	pub fn black_to_white() -> Self {
		Self::from(vec![Color::BLACK, Color::WHITE])
	}

	pub fn as_color_list(&self) -> &List<Color> {
		&self.0
	}

	pub fn into_color_list(self) -> List<Color> {
		self.0
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}

	pub fn iter(&self) -> GradientStopsIter<'_> {
		self.into_iter()
	}

	/// The color of the stop at the given index, if in bounds.
	pub fn color(&self, index: usize) -> Option<Color> {
		self.0.element(index).copied()
	}

	/// The effective position of the stop at the given index: its `position` attribute value, or its share of an even distribution when the attribute is absent.
	pub fn position(&self, index: usize) -> f64 {
		self.0.attribute::<f64>(ATTR_POSITION, index).copied().unwrap_or_else(|| even_position(index, self.len()))
	}

	/// The effective midpoint of the stop at the given index: its `midpoint` attribute value, or the linear interpolation default of `0.5` when the attribute is absent.
	pub fn midpoint(&self, index: usize) -> f64 {
		self.0.attribute::<f64>(ATTR_MIDPOINT, index).copied().unwrap_or(0.5)
	}

	/// The effective positions of all stops.
	pub fn positions(&self) -> Vec<f64> {
		(0..self.len()).map(|index| self.position(index)).collect()
	}

	/// The effective midpoints of all stops.
	pub fn midpoints(&self) -> Vec<f64> {
		(0..self.len()).map(|index| self.midpoint(index)).collect()
	}

	/// Whether the `position` attribute is explicitly present rather than falling back to the even distribution.
	pub fn has_position_attribute(&self) -> bool {
		self.0.iter_attribute_values::<f64>(ATTR_POSITION).is_some()
	}

	/// Whether the `midpoint` attribute is explicitly present rather than falling back to the linear interpolation default.
	pub fn has_midpoint_attribute(&self) -> bool {
		self.0.iter_attribute_values::<f64>(ATTR_MIDPOINT).is_some()
	}

	/// The `position` attribute's values when present, or `None` when the stops fall back to the even distribution.
	fn position_attribute(&self) -> Option<Vec<f64>> {
		self.0.iter_attribute_values::<f64>(ATTR_POSITION).map(|values| values.copied().collect())
	}

	/// The `midpoint` attribute's values when present, or `None` when the stops fall back to the linear interpolation default.
	fn midpoint_attribute(&self) -> Option<Vec<f64>> {
		self.0.iter_attribute_values::<f64>(ATTR_MIDPOINT).map(|values| values.copied().collect())
	}

	/// The `position` attribute when present and meaningfully different from the even distribution, which is the form worth persisting in the graph.
	pub fn nondefault_positions(&self) -> Option<Vec<f64>> {
		let positions = self.position_attribute()?;
		let count = self.len();
		positions
			.iter()
			.enumerate()
			.any(|(index, &position)| !position.is_finite() || (position - even_position(index, count)).abs() > 1e-6)
			.then_some(positions)
	}

	/// The `midpoint` attribute when present and meaningfully different from the linear interpolation default of `0.5`.
	pub fn nondefault_midpoints(&self) -> Option<Vec<f64>> {
		let midpoints = self.midpoint_attribute()?;
		midpoints.iter().any(|&midpoint| (midpoint - 0.5).abs() > 1e-6).then_some(midpoints)
	}

	/// Removes the `position`/`midpoint` attributes when they merely restate the defaults, restoring the canonical absence-as-default form.
	pub fn elide_default_attributes(&mut self) {
		if self.has_position_attribute() && self.nondefault_positions().is_none() {
			self.0.remove_attribute(ATTR_POSITION);
		}
		if self.has_midpoint_attribute() && self.nondefault_midpoints().is_none() {
			self.0.remove_attribute(ATTR_MIDPOINT);
		}
	}

	/// Writes the whole `position` attribute from the effective values, since the even-distribution default is index-dependent and can't be produced by cell-wise padding.
	fn materialize_default_positions(&mut self) {
		if self.has_position_attribute() {
			return;
		}

		let count = self.len();
		for index in 0..count {
			self.0.set_attribute(ATTR_POSITION, index, even_position(index, count));
		}
	}

	/// Replaces the color of the stop at `index`, if it exists.
	pub fn set_color(&mut self, index: usize, color: Color) {
		if let Some(element) = self.0.element_mut(index) {
			*element = color;
		}
	}

	/// Sets the position of the stop at `index`, if it exists, materializing the whole `position` attribute so the other stops keep their effective placements.
	pub fn set_position(&mut self, index: usize, position: f64) {
		if index >= self.len() {
			return;
		}
		self.materialize_default_positions();
		self.0.set_attribute(ATTR_POSITION, index, position);
	}

	/// Sets the midpoint of the stop at `index`, if it exists.
	pub fn set_midpoint(&mut self, index: usize, midpoint: f64) {
		if index >= self.len() {
			return;
		}
		self.0.set_attribute(ATTR_MIDPOINT, index, midpoint);
	}

	/// Replaces the `position` attribute with the given values, padding with the final value if fewer than the stop count and ignoring any extras.
	/// An empty list removes the attribute, restoring even distribution.
	pub fn set_positions(&mut self, positions: &[f64]) {
		let Some(&last) = positions.last() else {
			self.0.remove_attribute(ATTR_POSITION);
			return;
		};

		for index in 0..self.len() {
			self.0.set_attribute(ATTR_POSITION, index, positions.get(index).copied().unwrap_or(last));
		}
	}

	/// Replaces the `midpoint` attribute with the given values, padding with the final value if fewer than the stop count and ignoring any extras.
	/// An empty list removes the attribute, restoring the linear interpolation default of `0.5` for every stop.
	pub fn set_midpoints(&mut self, midpoints: &[f64]) {
		let Some(&last) = midpoints.last() else {
			self.0.remove_attribute(ATTR_MIDPOINT);
			return;
		};

		for index in 0..self.len() {
			self.0.set_attribute(ATTR_MIDPOINT, index, midpoints.get(index).copied().unwrap_or(last));
		}
	}

	/// Rebuilds the stop list from the given stop indices, preserving every attribute.
	fn reordered(&self, indices: impl IntoIterator<Item = usize>) -> List<Color> {
		let mut list = List::new();
		for index in indices {
			if let Some(item) = self.0.clone_item(index) {
				list.push(item);
			}
		}
		list
	}

	/// Remove a stop at the given index.
	pub fn remove(&mut self, index: usize) {
		self.0 = self.reordered((0..self.len()).filter(|&i| i != index));
	}

	/// Remove and return the last stop's color, or `None` if empty.
	pub fn pop(&mut self) -> Option<Color> {
		let color = self.color(self.len().checked_sub(1)?);
		self.0 = self.reordered(0..self.len() - 1);
		color
	}

	/// Move the stop at `index` to a new position, re-sorting the stops by position. Returns the new index of the moved stop.
	pub fn move_stop(&mut self, index: usize, position: f64) -> usize {
		if index >= self.len() {
			return index;
		}
		self.set_position(index, position);
		self.sort_returning_new_index(index)
	}

	/// Insert a new stop at the given position, sampling the gradient at that position to determine the new stop's color.
	/// The new stop's midpoint is inherited from the interval it splits (or `0.5` if inserting at the very start).
	/// Returns the index where the new stop was inserted.
	pub fn insert_stop(&mut self, position: f64) -> usize {
		let color = self.evaluate(position, Default::default());
		let index = (0..self.len()).position(|i| self.position(i) > position).unwrap_or(self.len());
		let midpoint = if index > 0 { self.midpoint(index - 1) } else { 0.5 };
		self.insert_stop_values(position, midpoint, color)
	}

	/// Insert a copy of the stop at `source_index` (same color and midpoint) at `position`, keeping the stops sorted by position.
	/// Returns the index where the copy was inserted, or `None` if `source_index` is out of range.
	pub fn duplicate_stop(&mut self, source_index: usize, position: f64) -> Option<usize> {
		let color = self.color(source_index)?;
		let midpoint = self.midpoint(source_index);
		Some(self.insert_stop_values(position, midpoint, color))
	}

	/// Splices a new stop into the sorted position, materializing explicit positions (an arbitrary insertion breaks even distribution)
	/// while giving the new stop a midpoint cell only if the attribute already exists.
	fn insert_stop_values(&mut self, position: f64, midpoint: f64, color: Color) -> usize {
		self.materialize_default_positions();
		let index = (0..self.len()).position(|i| self.position(i) > position).unwrap_or(self.len());

		let mut item = Item::new_from_element(color).with_attribute(ATTR_POSITION, position);
		if self.has_midpoint_attribute() {
			item = item.with_attribute(ATTR_MIDPOINT, midpoint);
		}

		let mut list = self.reordered(0..index);
		list.push(item);
		for i in index..self.len() {
			if let Some(existing) = self.0.clone_item(i) {
				list.push(existing);
			}
		}

		self.0 = list;
		index
	}

	/// Reset the midpoint for the interval starting at `index` to its default `0.5`.
	pub fn reset_midpoint(&mut self, index: usize) {
		if self.has_midpoint_attribute() && index < self.len() {
			self.0.set_attribute(ATTR_MIDPOINT, index, 0.5);
		}
	}

	/// Sort the stops in place by position; returns the new index of the stop that was at `previous_index` before sorting.
	fn sort_returning_new_index(&mut self, previous_index: usize) -> usize {
		// An absent position attribute is an even distribution, which is already sorted
		if !self.has_position_attribute() {
			return previous_index;
		}

		let mut indices: Vec<usize> = (0..self.len()).collect();
		indices.sort_by(|&a, &b| self.position(a).total_cmp(&self.position(b)));
		let new_index = indices.iter().position(|&i| i == previous_index).unwrap_or(previous_index);
		self.0 = self.reordered(indices);
		new_index
	}

	/// Gradient stops as evaluation and rendering should see them: positions clamped to the 0 to 1 range
	/// (infinities landing at the ends, a NaN dropping its stop from sampling since it has no defined placement)
	/// and sorted ascending, so the sampler and every renderer agree on how non-compliant authored data behaves.
	fn normalized_stops(&self) -> Vec<GradientStop> {
		let mut stops: Vec<GradientStop> = (0..self.len())
			.filter_map(|index| {
				let position = self.position(index).clamp(0., 1.);
				if position.is_nan() {
					return None;
				}

				let midpoint = self.midpoint(index);
				let color = self.color(index)?;

				Some(GradientStop { position, midpoint, color })
			})
			.collect();

		stops.sort_by(|a, b| a.position.total_cmp(&b.position));
		stops
	}

	/// Samples the gradient's color at `t`. Given a `t` outside the 0 to 1 range, the `spread_method` determines how the gradient extends.
	pub fn evaluate(&self, t: f64, spread_method: GradientSpreadMethod) -> Color {
		let t = match spread_method {
			GradientSpreadMethod::Pad => t.clamp(0., 1.),
			GradientSpreadMethod::Repeat => t.rem_euclid(1.),
			GradientSpreadMethod::Reflect => {
				let cycle = t.rem_euclid(2.);
				if cycle > 1. { 2. - cycle } else { cycle }
			}
		};

		let stops = self.normalized_stops();
		let (Some(first), Some(last)) = (stops.first(), stops.last()) else { return Color::BLACK };
		if t <= first.position {
			return first.color;
		}
		if t >= last.position {
			return last.color;
		}

		for pair in stops.windows(2) {
			let (a, b) = (&pair[0], &pair[1]);
			if t >= a.position && t <= b.position {
				let normalized_t = (t - a.position) / (b.position - a.position);
				let adjusted_t = apply_midpoint(normalized_t, a.midpoint);
				return a.color.lerp(&b.color, adjusted_t as f32);
			}
		}

		Color::BLACK
	}

	pub fn sort(&mut self) {
		self.sort_returning_new_index(0);
	}

	pub fn reversed(&self) -> Self {
		let count = self.len();
		let mut list = self.reordered((0..count).rev());

		// Row reversal already reversed the position cells' order, each also flips across the range
		if self.has_position_attribute()
			&& let Some(positions) = list.iter_attribute_values_mut::<f64>(ATTR_POSITION)
		{
			for position in positions {
				*position = 1. - *position;
			}
		}

		// Midpoints belong to the interval to a stop's right, so they shift by one stop as well as flipping
		if self.has_midpoint_attribute() {
			let midpoints: Vec<f64> = (0..count).map(|i| if i + 1 < count { 1. - self.midpoint(count - 2 - i) } else { 0.5 }).collect();
			for (index, midpoint) in midpoints.into_iter().enumerate() {
				list.set_attribute(ATTR_MIDPOINT, index, midpoint);
			}
		}

		Self(list)
	}

	pub fn map_colors<F: Fn(&Color) -> Color>(&self, f: F) -> Self {
		let mut mapped = self.clone();
		mapped.0.iter_element_values_mut().for_each(|color| *color = f(color));
		mapped
	}

	/// Build a CSS `linear-gradient(...)` string suitable for use as a `background-image`. Samples the midpoint curves so the rendered gradient matches Graphite's interpolation rather than browser defaults.
	pub fn to_css_linear_gradient(&self) -> String {
		if self.len() <= 1 {
			let hex = self.color(0).map(|c| SRGBA8::from(c).to_rgba_hex()).unwrap_or_else(|| "000000ff".to_string());
			return format!("linear-gradient(to right, #{hex} 0%, #{hex} 100%)");
		}
		let pieces = self
			.interpolated_samples()
			.into_iter()
			.map(|(position, color, _)| {
				let percent = ((position * 100.) * 1e2).round() / 1e2;
				format!("#{} {percent}%", SRGBA8::from(color).to_rgba_hex())
			})
			.collect::<Vec<_>>()
			.join(", ");
		format!("linear-gradient(to right, {pieces})")
	}

	/// Produce a set of linearly-interpolated color samples that approximate the gradient's midpoint curves.
	///
	/// Each sample is `(position, color, original_midpoint)` where `original_midpoint` is `Some(f64)` with the corresponding
	/// midpoint for actual gradient stops, and `None` for synthesized midpoint-curve approximation samples.
	///
	/// Interpolation is performed in sRGB gamma space (then lifted back to linear-light for output) because the downstream SVG/CSS
	/// renderer interpolates between adjacent `<stop>` colors in gamma space; doing the subdivision math in the same space ensures
	/// the chosen samples actually match the curve the browser will draw.
	pub fn interpolated_samples(&self) -> Vec<(f64, Color, Option<f64>)> {
		/// Controls accuracy vs. number of samples tradeoff.
		/// 2/255 means the linear approximation will deviate by no more than 2 gradations of 8-bit color from the theoretically perfect curve with this midpoint bias.
		const THRESHOLD: f64 = 2. / 255.;

		#[allow(clippy::too_many_arguments)]
		fn subdivide(left: f64, right: f64, midpoint: f64, pos_a: f64, pos_b: f64, color_a_gamma: [f32; 4], color_b_gamma: [f32; 4], result: &mut Vec<(f64, Color, Option<f64>)>, depth: u32) {
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
				subdivide(left, mid, midpoint, pos_a, pos_b, color_a_gamma, color_b_gamma, result, depth + 1);

				let global_pos = pos_a + mid * (pos_b - pos_a);
				let t = y_actual as f32;
				let r = color_a_gamma[0] + (color_b_gamma[0] - color_a_gamma[0]) * t;
				let g = color_a_gamma[1] + (color_b_gamma[1] - color_a_gamma[1]) * t;
				let b = color_a_gamma[2] + (color_b_gamma[2] - color_a_gamma[2]) * t;
				let a = color_a_gamma[3] + (color_b_gamma[3] - color_a_gamma[3]) * t;
				let color = Color::from_gamma_srgb_channels(r, g, b, a);
				result.push((global_pos, color, None));

				subdivide(mid, right, midpoint, pos_a, pos_b, color_a_gamma, color_b_gamma, result, depth + 1);
			}
		}

		let stops = self.normalized_stops();
		let count = stops.len();
		if count == 0 {
			return vec![];
		}

		if count == 1 {
			return vec![(stops[0].position, stops[0].color, Some(sanitized_midpoint(stops[0].midpoint)))];
		}

		let mut result = Vec::new();

		for i in 0..count - 1 {
			let pos_a = stops[i].position;
			let pos_b = stops[i + 1].position;
			let color_a = stops[i].color;
			let color_b = stops[i + 1].color;
			let midpoint = sanitized_midpoint(stops[i].midpoint);
			let next_midpoint = sanitized_midpoint(stops[i + 1].midpoint);

			// Add the start stop (subsequent segments share the previous end stop)
			if i == 0 {
				result.push((pos_a, color_a, Some(midpoint)));
			}

			// Only subdivide if midpoint deviates from linear (0.5)
			if (midpoint - 0.5).abs() >= 1e-6 {
				subdivide(0., 1., midpoint, pos_a, pos_b, color_a.to_gamma_srgb_channels(), color_b.to_gamma_srgb_channels(), &mut result, 0);
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

	pub fn lerp(&self, other: &Self, time: f64) -> Self {
		let stops = self.iter().zip(other.iter()).map(|(a, b)| {
			let position = a.position + (b.position - a.position) * time;
			let color = a.color.lerp(&b.color, time as f32);
			GradientStop { position, midpoint: 0.5, color }
		});
		Gradient::new(stops)
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientSpreadMethod {
	#[default]
	Pad,
	Reflect,
	Repeat,
	// TODO: Add a "Clear" variant that returns transparent black outside the gradient's range
}

impl GradientSpreadMethod {
	pub fn svg_name(&self) -> &'static str {
		match self {
			GradientSpreadMethod::Pad => "pad",
			GradientSpreadMethod::Reflect => "reflect",
			GradientSpreadMethod::Repeat => "repeat",
		}
	}
}

/// Rebuild the y-axis so its (parallel, perpendicular) components in the x-axis-aligned frame stay constant, both
/// rescaled by `|new_x| / |old_x|`. This holds the (x, y) parallelogram's aspect ratio and skew fixed across an endpoint
/// drag, so a radial ellipse stays the same shape (just rotated and resized) instead of distorting as x grows or shrinks.
/// Falls back to a +90° rotation of `new_x` when `old_x` is degenerate.
fn scale_y_axis_to_match_new_x(old_x: DVec2, old_y: DVec2, new_x: DVec2) -> DVec2 {
	let old_x_length = old_x.length();
	if old_x_length < 1e-9 {
		return DVec2::new(-new_x.y, new_x.x);
	}
	let ex_old = old_x / old_x_length;
	let ey_old = DVec2::new(-ex_old.y, ex_old.x);

	let new_x_length = new_x.length();
	if new_x_length < 1e-9 {
		return DVec2::ZERO;
	}
	let ex_new = new_x / new_x_length;
	let ey_new = DVec2::new(-ex_new.y, ex_new.x);

	let parallel = old_y.dot(ex_old);
	let perpendicular = old_y.dot(ey_old);
	let scale = new_x_length / old_x_length;

	scale * (parallel * ex_new + perpendicular * ey_new)
}

/// Build a new affine that maps canonical (0,0) -> (1,0) to (new_start, new_end), preserving the y-axis
/// shape of `old` proportionally to the x-axis length change.
pub fn build_transform_with_y_preservation(old: DAffine2, new_start: DVec2, new_end: DVec2) -> DAffine2 {
	let new_x_axis = new_end - new_start;
	let preserved_y_axis = scale_y_axis_to_match_new_x(old.matrix2.x_axis, old.matrix2.y_axis, new_x_axis);
	DAffine2 {
		matrix2: glam::DMat2::from_cols(new_x_axis, preserved_y_axis),
		translation: new_start,
	}
}

/// Build the default transform for a gradient not yet given one: a horizontal gradient spanning the
/// bounding box's width, running through its vertical middle.
pub fn initial_gradient_transform_for_bounding_box(bounds: [DVec2; 2]) -> DAffine2 {
	let [min, max] = bounds;
	let x_axis = DVec2::new(max.x - min.x, 0.);
	DAffine2 {
		matrix2: glam::DMat2::from_cols(x_axis, x_axis.perp()),
		translation: DVec2::new(min.x, (min.y + max.y) / 2.),
	}
}

impl core_types::bounds::BoundingBox for Gradient {
	fn bounding_box(&self, _transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		core_types::bounds::RenderBoundingBox::Infinite
	}

	fn thumbnail_bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> core_types::bounds::RenderBoundingBox {
		// AABB of the gradient line itself, leaving aspect padding and sub-pixel fallbacks to the runtime so this stays
		// a clean per-item geometric bound that combines naturally with siblings
		let start = transform.transform_point2(DVec2::ZERO);
		let end = transform.transform_point2(DVec2::X);
		core_types::bounds::RenderBoundingBox::Rectangle([start.min(end), start.max(end)])
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn default_is_empty_and_black_to_white_is_the_artist_starting_gradient() {
		assert!(Gradient::default().is_empty());
		assert_eq!(Gradient::black_to_white().positions(), vec![0., 1.]);
		assert_eq!(Gradient::default().evaluate(0.5, Default::default()), Color::BLACK);
	}

	#[test]
	fn absent_attributes_default_to_even_positions_and_linear_midpoints() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		assert_eq!(gradient.positions(), vec![0., 0.5, 1.]);
		assert_eq!(gradient.midpoints(), vec![0.5, 0.5, 0.5]);
	}

	#[test]
	fn serde_round_trip_preserves_attribute_absence() {
		let implicit = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&implicit).unwrap();
		assert!(!json.contains("position") && !json.contains("midpoint"), "absent attributes must not serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), implicit);

		let mut explicit = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		explicit.set_positions(&[0.2, 0.9]);
		explicit.set_midpoints(&[0.3, 0.5]);
		let explicit = GradientRamp::from(explicit);
		let json = serde_json::to_string(&explicit).unwrap();
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), explicit);
	}

	#[test]
	fn gradient_ui_write_back_elides_default_attributes() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		gradient.set_midpoints(&[0.7, 0.5, 0.5]);

		let round_tripped = Gradient::from(&GradientStops::<SRGBA8>::from(&gradient));
		assert!(!round_tripped.has_position_attribute(), "materialized even positions should elide on write-back");
		assert_eq!(round_tripped.midpoints(), vec![0.7, 0.5, 0.5]);
	}

	#[test]
	fn nondefault_attributes_elide_default_values() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE, Color::RED]);
		assert_eq!(gradient.nondefault_positions(), None);
		assert_eq!(gradient.nondefault_midpoints(), None);

		// Explicit attributes that merely restate the defaults still elide
		gradient.set_positions(&[0., 0.5, 1.]);
		gradient.set_midpoints(&[0.5, 0.5, 0.5]);
		assert_eq!(gradient.nondefault_positions(), None);
		assert_eq!(gradient.nondefault_midpoints(), None);

		gradient.set_positions(&[0., 0.25, 1.]);
		gradient.set_midpoints(&[0.5, 0.7, 0.5]);
		assert_eq!(gradient.nondefault_positions(), Some(vec![0., 0.25, 1.]));
		assert_eq!(gradient.nondefault_midpoints(), Some(vec![0.5, 0.7, 0.5]));
	}

	#[test]
	fn non_compliant_positions_normalize_for_sampling_and_rendering() {
		// Stored positions stay as authored, but consumers see them clamped to the 0 to 1 range and sorted
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[1.5, 0.4, -0.5]);
		assert_eq!(gradient.positions(), vec![1.5, 0.4, -0.5]);

		let sample_positions: Vec<f64> = gradient.interpolated_samples().iter().map(|(position, ..)| *position).collect();
		assert!(sample_positions.windows(2).all(|pair| pair[0] <= pair[1]), "samples must ascend: {sample_positions:?}");
		assert_eq!(sample_positions.first(), Some(&0.));
		assert_eq!(sample_positions.last(), Some(&1.));

		assert_eq!(gradient.evaluate(0., Default::default()), Color::RED);
		assert_eq!(gradient.evaluate(1., Default::default()), Color::WHITE);
	}

	#[test]
	fn infinite_positions_clamp_to_the_range_ends() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[f64::INFINITY, f64::NEG_INFINITY]);

		let sample_positions: Vec<f64> = gradient.interpolated_samples().iter().map(|(position, ..)| *position).collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(gradient.evaluate(0., Default::default()), Color::BLACK);
		assert_eq!(gradient.evaluate(1., Default::default()), Color::WHITE);
	}

	#[test]
	fn nan_positions_drop_their_stops_from_sampling() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[0., f64::NAN, 1.]);

		let sample_positions: Vec<f64> = gradient.interpolated_samples().iter().map(|(position, ..)| *position).collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(gradient.evaluate(0.5, Default::default()), Color::WHITE.lerp(&Color::RED, 0.5));

		// A non-finite position is preserved as nondefault so write-back elision cannot resurrect the dropped stop
		assert!(gradient.nondefault_positions().is_some());

		// With every position NaN the gradient samples as stopless, painting solid black to signal the upstream bug
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::RED]);
		gradient.set_positions(&[f64::NAN, f64::NAN]);
		assert!(gradient.interpolated_samples().is_empty());
		assert_eq!(gradient.evaluate(0.5, Default::default()), Color::BLACK);
	}

	#[test]
	fn samples_start_at_the_first_stop_without_synthetic_lead_in() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[0.3, 1.]);

		let samples = gradient.interpolated_samples();
		assert_eq!(samples[0], (0.3, Color::WHITE, None), "renderers that need a flat lead-in before the first stop add it themselves");
	}

	#[test]
	fn nan_midpoints_read_as_linear() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		let linear_result = gradient.evaluate(0.25, Default::default());

		gradient.set_midpoints(&[f64::NAN, f64::NAN]);
		assert_eq!(gradient.evaluate(0.25, Default::default()), linear_result);
		let no_nan_annotations = gradient
			.interpolated_samples()
			.iter()
			.all(|(position, _, midpoint)| position.is_finite() && !midpoint.is_some_and(|midpoint| midpoint.is_nan()));
		assert!(no_nan_annotations, "NaN must not escape into rendered sample annotations");
	}
}
