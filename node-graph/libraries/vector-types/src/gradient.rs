use core_types::Color;
use core_types::color::SRGBA8;
use core_types::list::{ATTR_GRADIENT_HUE_DIRECTION, ATTR_GRADIENT_INTERPOLATION, ATTR_GRADIENT_SPREAD, ATTR_MIDPOINT, ATTR_POSITION, Item, List};
use core_types::render_complexity::RenderComplexity;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Radio)]
pub enum GradientForm {
	/// Transitions the colors along a straight line.
	#[default]
	Linear,
	/// Transitions the colors outward from a center point.
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
	pub fn to_css_linear_gradient(&self, gradient_interpolation: GradientInterpolation, gradient_hue_direction: GradientHueDirection) -> String {
		Gradient::from(self).to_css_linear_gradient(gradient_interpolation, gradient_hue_direction)
	}
}

/// The serialized exchange form of a gradient: its stops, with whole-ramp settings as sibling fields serialized
/// only when non-default. The interpolation is the exception: it always serializes, so its absence marks a ramp
/// from before the field existed, which deserializes as the gamma those documents rendered with.
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, Debug, Clone, PartialEq, graphene_hash::CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradientRamp<C = Color> {
	pub stops: GradientStops<C>,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientSpread::is_default"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_spread: GradientSpread,
	// TODO: Elide the default again (removing `legacy_gamma`) when switching to the new document format and Ctrl-C node serialization format
	#[cfg_attr(feature = "serde", serde(default = "GradientInterpolation::legacy_gamma"))]
	pub gradient_interpolation: GradientInterpolation,
	#[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "GradientHueDirection::is_default"))]
	#[cfg_attr(feature = "wasm", tsify(optional))]
	pub gradient_hue_direction: GradientHueDirection,
}

unsafe impl<C: dyn_any::StaticTypeSized> dyn_any::StaticType for GradientRamp<C> {
	type Static = GradientRamp<C::Static>;
}

impl<C> From<GradientStops<C>> for GradientRamp<C> {
	fn from(stops: GradientStops<C>) -> Self {
		Self {
			stops,
			gradient_spread: Default::default(),
			gradient_interpolation: Default::default(),
			gradient_hue_direction: Default::default(),
		}
	}
}

impl From<&Gradient> for GradientRamp {
	fn from(gradient: &Gradient) -> Self {
		Self {
			stops: gradient.into(),
			gradient_spread: Default::default(),
			gradient_interpolation: Default::default(),
			gradient_hue_direction: Default::default(),
		}
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

// The runtime wire form: whole-ramp settings ride as the gradient item's attributes in its containing list,
// where the Fill kernel, chain setter nodes, and renderers read and write them
impl From<GradientRamp> for Item<Gradient> {
	fn from(ramp: GradientRamp) -> Self {
		let mut item = Item::new_from_element(Gradient::from(ramp.stops));
		if !ramp.gradient_spread.is_default() {
			item.set_attribute(ATTR_GRADIENT_SPREAD, ramp.gradient_spread);
		}
		if !ramp.gradient_interpolation.is_default() {
			item.set_attribute(ATTR_GRADIENT_INTERPOLATION, ramp.gradient_interpolation);
		}
		if !ramp.gradient_hue_direction.is_default() {
			item.set_attribute(ATTR_GRADIENT_HUE_DIRECTION, ramp.gradient_hue_direction);
		}
		item
	}
}

impl From<&Item<Gradient>> for GradientRamp {
	fn from(item: &Item<Gradient>) -> Self {
		Self {
			stops: item.element().into(),
			gradient_spread: item.attribute_cloned_or_default(ATTR_GRADIENT_SPREAD),
			gradient_interpolation: item.attribute_cloned_or_default(ATTR_GRADIENT_INTERPOLATION),
			gradient_hue_direction: item.attribute_cloned_or_default(ATTR_GRADIENT_HUE_DIRECTION),
		}
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

impl From<&GradientRamp> for GradientRamp<SRGBA8> {
	fn from(ramp: &GradientRamp) -> Self {
		Self {
			stops: ramp.into(),
			gradient_spread: ramp.gradient_spread,
			gradient_interpolation: ramp.gradient_interpolation,
			gradient_hue_direction: ramp.gradient_hue_direction,
		}
	}
}

impl From<&Gradient> for GradientRamp<SRGBA8> {
	fn from(gradient: &Gradient) -> Self {
		Self {
			stops: gradient.into(),
			gradient_spread: Default::default(),
			gradient_interpolation: Default::default(),
			gradient_hue_direction: Default::default(),
		}
	}
}

impl From<&GradientRamp<SRGBA8>> for GradientRamp {
	fn from(ramp: &GradientRamp<SRGBA8>) -> Self {
		Self {
			gradient_spread: ramp.gradient_spread,
			gradient_interpolation: ramp.gradient_interpolation,
			gradient_hue_direction: ramp.gradient_hue_direction,
			..Self::from(&ramp.stops)
		}
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

/// Interpolates between two adjacent stops' colors at `t` across their interval, in the gradient's interpolation color space.
pub fn interpolate_stop_colors(color_a: Color, color_b: Color, t: f32, gradient_interpolation: GradientInterpolation, gradient_hue_direction: GradientHueDirection) -> Color {
	match gradient_interpolation {
		GradientInterpolation::OkLab => lerp_in_space::<color::Oklab>(color_a, color_b, t, gradient_hue_direction),
		GradientInterpolation::OkLch => lerp_in_space::<color::Oklch>(color_a, color_b, t, gradient_hue_direction),
		GradientInterpolation::Lab => lerp_in_space::<color::Lab>(color_a, color_b, t, gradient_hue_direction),
		GradientInterpolation::LCh => lerp_in_space::<color::Lch>(color_a, color_b, t, gradient_hue_direction),
		GradientInterpolation::Hsl => lerp_in_space::<color::Hsl>(color_a, color_b, t, gradient_hue_direction),
		GradientInterpolation::SrgbLinear => color_a.lerp(&color_b, t),
		GradientInterpolation::SrgbGamma => color_a.lerp_gamma_srgb(&color_b, t),
	}
}

/// Mix two colors in the color space `CS`, with alpha blending linearly like the sRGB spaces.
/// Polar spaces arc through hue per the direction, with an achromatic endpoint's powerless hue adopting the other's per CSS.
/// The mix can land slightly outside the sRGB gamut; it stays unclamped here and clips at render encoding.
fn lerp_in_space<CS: color::ColorSpace>(color_a: Color, color_b: Color, t: f32, gradient_hue_direction: GradientHueDirection) -> Color {
	use color::ColorSpaceLayout;

	let mut a = CS::from_linear_srgb([color_a.r(), color_a.g(), color_a.b()]);
	let mut b = CS::from_linear_srgb([color_b.r(), color_b.g(), color_b.b()]);

	let hue_index = match CS::LAYOUT {
		ColorSpaceLayout::HueFirst => Some(0),
		ColorSpaceLayout::HueThird => Some(2),
		_ => None,
	};
	if let Some(hue_index) = hue_index {
		// Chroma (or saturation) is channel 1 in both polar layouts; the threshold scales to the space's
		// lightness range so conversion noise on achromatic colors stays below it
		let achromatic = 1e-4 * CS::WHITE_COMPONENTS.iter().fold(0_f32, |max, &component| max.max(component));
		if a[1] < achromatic && b[1] >= achromatic {
			a[hue_index] = b[hue_index];
		}
		if b[1] < achromatic && a[1] >= achromatic {
			b[hue_index] = a[hue_index];
		}

		// The CSS Color 4 hue fixup, on hues the conversions already place in the 0 to 360 range
		let delta = b[hue_index] - a[hue_index];
		let delta = match gradient_hue_direction {
			GradientHueDirection::Shorter => {
				if delta > 180. {
					delta - 360.
				} else if delta < -180. {
					delta + 360.
				} else {
					delta
				}
			}
			GradientHueDirection::Longer => {
				if 0. < delta && delta < 180. {
					delta - 360.
				} else if -180. < delta && delta <= 0. {
					delta + 360.
				} else {
					delta
				}
			}
			GradientHueDirection::Increasing => {
				if delta < 0. {
					delta + 360.
				} else {
					delta
				}
			}
			GradientHueDirection::Decreasing => {
				if delta > 0. {
					delta - 360.
				} else {
					delta
				}
			}
		};
		b[hue_index] = a[hue_index] + delta;
	}

	let mixed = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];

	let [red, green, blue] = CS::to_linear_srgb(mixed);
	Color::from_rgbaf32_unchecked(red, green, blue, color_a.a() + (color_b.a() - color_a.a()) * t)
}

/// The largest difference between two colors across their gamma sRGB channels, the 8-bit-adjacent measure that rendered output quantizes to.
fn max_gamma_channel_deviation(a: Color, b: Color) -> f64 {
	let (a, b) = (a.to_gamma_srgb_channels(), b.to_gamma_srgb_channels());
	(0..4).fold(0_f64, |max, i| max.max((a[i] - b[i]).abs() as f64))
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
	pub fn insert_stop(&mut self, position: f64, gradient_interpolation: GradientInterpolation, gradient_hue_direction: GradientHueDirection) -> usize {
		let color = self.evaluate(position, Default::default(), gradient_interpolation, gradient_hue_direction);
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

	/// Samples the gradient's color at `t`. Given a `t` outside the 0 to 1 range, the `gradient_spread` determines how the gradient extends.
	pub fn evaluate(&self, t: f64, gradient_spread: GradientSpread, gradient_interpolation: GradientInterpolation, gradient_hue_direction: GradientHueDirection) -> Color {
		let t = match gradient_spread {
			GradientSpread::Pad => t.clamp(0., 1.),
			GradientSpread::Repeat => t.rem_euclid(1.),
			GradientSpread::Reflect => {
				let cycle = t.rem_euclid(2.);
				if cycle > 1. { 2. - cycle } else { cycle }
			}
			GradientSpread::Clear => {
				if !(0. ..=1.).contains(&t) {
					return Color::TRANSPARENT;
				}
				t
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
				return interpolate_stop_colors(a.color, b.color, adjusted_t as f32, gradient_interpolation, gradient_hue_direction);
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

	/// Build a CSS `linear-gradient(...)` string suitable for use as a `background-image`. Samples the midpoint curves and interpolation color space so the rendered gradient matches Graphite's interpolation rather than browser defaults.
	pub fn to_css_linear_gradient(&self, gradient_interpolation: GradientInterpolation, gradient_hue_direction: GradientHueDirection) -> String {
		if self.len() <= 1 {
			let hex = self.color(0).map(|c| SRGBA8::from(c).to_rgba_hex()).unwrap_or_else(|| "000000ff".to_string());
			return format!("linear-gradient(to right, #{hex} 0%, #{hex} 100%)");
		}
		let pieces = self
			.interpolated_samples(gradient_interpolation, gradient_hue_direction)
			.into_iter()
			.map(|(position, color, _)| {
				let percent = ((position * 100.) * 1e2).round() / 1e2;
				format!("#{} {percent}%", SRGBA8::from(color).to_rgba_hex())
			})
			.collect::<Vec<_>>()
			.join(", ");
		format!("linear-gradient(to right, {pieces})")
	}

	/// Produce a set of linearly-interpolated color samples that approximate the gradient's midpoint curves
	/// and interpolation color space.
	///
	/// Each sample is `(position, color, original_midpoint)` where `original_midpoint` is `Some(f64)` with the corresponding
	/// midpoint for actual gradient stops, and `None` for synthesized curve approximation samples.
	///
	/// The downstream SVG/CSS and Vello renderers interpolate between adjacent emitted stops in gamma sRGB space, so the
	/// subdivision emits enough samples that the gamma-drawn segments match the ramp's true curve: the midpoint bias, and
	/// the interpolation color space when it is not gamma itself.
	pub fn interpolated_samples(&self, gradient_interpolation: GradientInterpolation, gradient_hue_direction: GradientHueDirection) -> Vec<(f64, Color, Option<f64>)> {
		/// Controls accuracy vs. number of samples tradeoff.
		/// 2/255 means the linear approximation will deviate by no more than 2 gradations of 8-bit color from the theoretically perfect curve with this midpoint bias.
		const THRESHOLD: f64 = 2. / 255.;

		#[allow(clippy::too_many_arguments)]
		fn subdivide(
			left: f64,
			right: f64,
			midpoint: f64,
			pos_a: f64,
			pos_b: f64,
			color_a: Color,
			color_b: Color,
			gradient_interpolation: GradientInterpolation,
			gradient_hue_direction: GradientHueDirection,
			result: &mut Vec<(f64, Color, Option<f64>)>,
			depth: u32,
		) {
			const MAX_DEPTH: u32 = 20;
			if depth >= MAX_DEPTH {
				return;
			}

			let mid = (left + right) / 2.;

			let y_actual = apply_midpoint(mid, midpoint);
			let y_left = apply_midpoint(left, midpoint);
			let y_right = apply_midpoint(right, midpoint);
			let y_linear = (y_left + y_right) / 2.;

			// A sample is needed wherever the renderer's gamma segment between the flanking samples would stray
			// from the ramp's true curve: from the midpoint bias, or from a non-gamma space's own curvature.
			// The space check probes the quarter points as well as the center, since spaces with a steep toe
			// (like CIE Lab near black) peak their deviation off-center
			let midpoint_deviates = (y_actual - y_linear).abs() > THRESHOLD;
			let space_deviates = gradient_interpolation != GradientInterpolation::SrgbGamma && {
				let color_left = interpolate_stop_colors(color_a, color_b, y_left as f32, gradient_interpolation, gradient_hue_direction);
				let color_right = interpolate_stop_colors(color_a, color_b, y_right as f32, gradient_interpolation, gradient_hue_direction);
				[0.25, 0.5, 0.75].into_iter().any(|fraction| {
					let y_probe = apply_midpoint(left + (right - left) * fraction, midpoint);
					let color_target = interpolate_stop_colors(color_a, color_b, y_probe as f32, gradient_interpolation, gradient_hue_direction);
					max_gamma_channel_deviation(color_target, color_left.lerp_gamma_srgb(&color_right, fraction as f32)) > THRESHOLD
				})
			};

			if midpoint_deviates || space_deviates {
				subdivide(left, mid, midpoint, pos_a, pos_b, color_a, color_b, gradient_interpolation, gradient_hue_direction, result, depth + 1);

				let global_pos = pos_a + mid * (pos_b - pos_a);
				let color = interpolate_stop_colors(color_a, color_b, y_actual as f32, gradient_interpolation, gradient_hue_direction);
				result.push((global_pos, color, None));

				subdivide(mid, right, midpoint, pos_a, pos_b, color_a, color_b, gradient_interpolation, gradient_hue_direction, result, depth + 1);
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

			// Only subdivide if the midpoint deviates from linear (0.5) or a non-gamma space may curve away from the drawn gamma segment
			if (midpoint - 0.5).abs() >= 1e-6 || gradient_interpolation != GradientInterpolation::SrgbGamma {
				subdivide(0., 1., midpoint, pos_a, pos_b, color_a, color_b, gradient_interpolation, gradient_hue_direction, &mut result, 0);
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
pub enum GradientSpread {
	/// Extends the end colors outward.
	#[default]
	#[icon("GradientSpreadPad")]
	Pad,
	/// Loops the gradient by mirroring back-and-forth.
	#[icon("GradientSpreadReflect")]
	Reflect,
	/// Loops the gradient as copies of itself.
	#[icon("GradientSpreadRepeat")]
	Repeat,
	/// Cuts off to transparency beyond the ends.
	#[icon("GradientSpreadClear")]
	Clear,
}

impl GradientSpread {
	pub fn svg_name(&self) -> &'static str {
		match self {
			GradientSpread::Pad => "pad",
			GradientSpread::Reflect => "reflect",
			GradientSpread::Repeat => "repeat",
			// SVG has no clear mode; renderers emulate it over pad with transparent guard stops
			GradientSpread::Clear => "pad",
		}
	}

	pub fn is_default(&self) -> bool {
		*self == Self::default()
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum GradientInterpolation {
	/// Blends stops in the OkLab perceptual color space, keeping transitions visually even.
	#[default]
	#[label("OkLab")]
	OkLab,
	/// Blends stops in the polar form of OkLab, arcing through hue instead of fading through gray.
	#[label("OkLch")]
	OkLch,
	/// Blends stops in the CIE Lab color space, the classic perceptual standard.
	Lab,
	/// Blends stops in the polar form of CIE Lab, arcing through hue instead of fading through gray.
	#[label("LCh")]
	LCh,
	/// Blends stops in the classic hue, saturation, and lightness cylinder.
	#[label("HSL")]
	Hsl,
	/// Blends stops in linear light, keeping transitions evenly bright.
	#[label("sRGB Linear")]
	SrgbLinear,
	/// Blends stops in gamma-encoded sRGB, the classic SVG and CSS look.
	#[label("sRGB Gamma")]
	SrgbGamma,
}

impl GradientInterpolation {
	pub fn is_default(&self) -> bool {
		*self == Self::default()
	}

	/// Whether the space is polar (cylindrical), making the hue direction option meaningful.
	pub fn is_polar(&self) -> bool {
		matches!(self, Self::OkLch | Self::LCh | Self::Hsl)
	}

	// TODO: Remove when switching to the new document format and Ctrl-C node serialization format
	fn legacy_gamma() -> Self {
		Self::SrgbGamma
	}
}

#[repr(C)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug, Hash, graphene_hash::CacheHash, DynAny, node_macro::ChoiceType)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[widget(Dropdown)]
pub enum GradientHueDirection {
	/// Blends across the shorter arc around the hue wheel.
	#[default]
	Shorter,
	/// Blends across the longer arc around the hue wheel.
	Longer,
	/// Blends with the hue angle always increasing.
	Increasing,
	/// Blends with the hue angle always decreasing.
	Decreasing,
}

impl GradientHueDirection {
	pub fn is_default(&self) -> bool {
		*self == Self::default()
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
		assert_eq!(Gradient::default().evaluate(0.5, Default::default(), Default::default(), Default::default()), Color::BLACK);
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
	fn gradient_spread_serializes_only_when_not_default() {
		let default_spread = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&default_spread).unwrap();
		assert!(!json.contains("gradient_spread"), "the default Pad gradient spread must not serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), default_spread);

		let repeating = GradientRamp {
			gradient_spread: GradientSpread::Repeat,
			..default_spread.clone()
		};
		let json = serde_json::to_string(&repeating).unwrap();
		assert!(json.contains(r#""gradient_spread":"Repeat""#), "a non-default gradient spread must serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), repeating);
	}

	#[test]
	fn gradient_spread_round_trips_through_the_item_attribute() {
		let ramp = GradientRamp {
			gradient_spread: GradientSpread::Repeat,
			..GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]))
		};

		let item = Item::<Gradient>::from(ramp.clone());
		assert_eq!(
			item.attribute_cloned_or_default::<GradientSpread>(ATTR_GRADIENT_SPREAD),
			GradientSpread::Repeat,
			"the runtime item should carry the gradient spread as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), ramp);

		let padded = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			padded.attribute::<GradientSpread>(ATTR_GRADIENT_SPREAD).is_none(),
			"the default Pad must stay absent rather than materialize"
		);
	}

	#[test]
	fn gradient_interpolation_always_serializes_and_its_absence_reads_as_legacy_gamma() {
		let default_interpolation = GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]));
		let json = serde_json::to_string(&default_interpolation).unwrap();
		assert!(
			json.contains(r#""gradient_interpolation":"OkLab""#),
			"the interpolation must serialize even at its default, marking the ramp as post-legacy: {json}"
		);
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), default_interpolation);

		let gamma = GradientRamp {
			gradient_interpolation: GradientInterpolation::SrgbGamma,
			..default_interpolation.clone()
		};
		let json = serde_json::to_string(&gamma).unwrap();
		assert!(json.contains(r#""gradient_interpolation":"SrgbGamma""#), "a non-default interpolation must serialize: {json}");
		assert_eq!(serde_json::from_str::<GradientRamp>(&json).unwrap(), gamma);

		let legacy_json = json.replace(r#","gradient_interpolation":"SrgbGamma""#, "");
		assert_eq!(
			serde_json::from_str::<GradientRamp>(&legacy_json).unwrap(),
			gamma,
			"a ramp saved before the field existed should read as the gamma it rendered with: {legacy_json}"
		);
	}

	#[test]
	fn gradient_interpolation_round_trips_through_the_item_attribute() {
		let ramp = GradientRamp {
			gradient_interpolation: GradientInterpolation::SrgbGamma,
			..GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE]))
		};

		let item = Item::<Gradient>::from(ramp.clone());
		assert_eq!(
			item.attribute_cloned_or_default::<GradientInterpolation>(ATTR_GRADIENT_INTERPOLATION),
			GradientInterpolation::SrgbGamma,
			"the runtime item should carry the interpolation as its attribute"
		);
		assert_eq!(GradientRamp::from(&item), ramp);

		let linear = Item::<Gradient>::from(GradientRamp::from(Gradient::from(vec![Color::BLACK, Color::WHITE])));
		assert!(
			linear.attribute::<GradientInterpolation>(ATTR_GRADIENT_INTERPOLATION).is_none(),
			"the default Linear must stay absent rather than materialize"
		);
	}

	#[test]
	fn linear_interpolation_densifies_samples_where_gamma_segments_deviate() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		// Gamma needs no synthesized samples since the renderers already draw gamma segments
		assert_eq!(gradient.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default()).len(), 2);

		// A linear black-to-white ramp curves away from any single gamma segment, so samples must densify,
		// keeping the end stops in place and every synthesized color on the linear-light line
		let samples = gradient.interpolated_samples(GradientInterpolation::SrgbLinear, Default::default());
		assert!(samples.len() > 2, "linear interpolation should synthesize samples, got {}", samples.len());
		assert_eq!(samples.first().unwrap().0, 0.);
		assert_eq!(samples.last().unwrap().0, 1.);
		for &(position, color, _) in &samples {
			assert!(
				(color.r() as f64 - position).abs() < 1e-5,
				"sample at {position} should sit on the linear-light line, got {}",
				color.r()
			);
		}

		// Identical end colors leave nothing to densify
		let flat = Gradient::from(vec![Color::WHITE, Color::WHITE]);
		assert_eq!(flat.interpolated_samples(GradientInterpolation::SrgbLinear, Default::default()).len(), 2);
	}

	#[test]
	fn midpoint_bias_and_interpolation_space_compose_within_playback_tolerance() {
		let color_pairs = [
			(Color::BLACK, Color::WHITE),
			(Color::RED, Color::WHITE),
			(Color::from_rgbaf32_unchecked(0.9, 0.2, 0.05, 1.), Color::from_rgbaf32_unchecked(0.05, 0.3, 0.8, 0.5)),
		];

		// Sweep the midpoint against each space so its bias and the space's curvature also oppose each other,
		// asserting the emitted samples' gamma playback tracks the composed midpoint-then-space theoretical curve
		for (gradient_interpolation, gradient_hue_direction) in [
			(GradientInterpolation::OkLab, GradientHueDirection::Shorter),
			(GradientInterpolation::OkLch, GradientHueDirection::Shorter),
			(GradientInterpolation::OkLch, GradientHueDirection::Longer),
			(GradientInterpolation::Lab, GradientHueDirection::Shorter),
			(GradientInterpolation::LCh, GradientHueDirection::Shorter),
			(GradientInterpolation::Hsl, GradientHueDirection::Shorter),
			(GradientInterpolation::Hsl, GradientHueDirection::Longer),
			(GradientInterpolation::SrgbLinear, GradientHueDirection::Shorter),
			(GradientInterpolation::SrgbGamma, GradientHueDirection::Shorter),
		] {
			for &(color_a, color_b) in &color_pairs {
				for midpoint_step in 1..40 {
					let midpoint = midpoint_step as f64 / 40.;

					let mut gradient = Gradient::from(vec![color_a, color_b]);
					gradient.set_midpoints(&[midpoint, 0.5]);
					let samples = gradient.interpolated_samples(gradient_interpolation, gradient_hue_direction);

					for probe in 0..=1000 {
						let t = probe as f64 / 1000.;

						let after = samples.iter().position(|&(position, ..)| position >= t).unwrap_or(samples.len() - 1);
						let playback = if after == 0 {
							samples[0].1
						} else {
							let (left_position, left_color, _) = samples[after - 1];
							let (right_position, right_color, _) = samples[after];
							let span = right_position - left_position;
							if span < 1e-12 {
								right_color
							} else {
								left_color.lerp_gamma_srgb(&right_color, ((t - left_position) / span) as f32)
							}
						};

						let true_color = interpolate_stop_colors(color_a, color_b, apply_midpoint(t, midpoint) as f32, gradient_interpolation, gradient_hue_direction);
						let deviation = max_gamma_channel_deviation(playback, true_color);
						assert!(
							deviation <= 4. / 255.,
							"playback deviates {:.1}/255 at t={t} with midpoint {midpoint} in {gradient_interpolation:?} ({gradient_hue_direction:?}) between {color_a:?} and {color_b:?}",
							deviation * 255.
						);
					}
				}
			}
		}
	}

	#[test]
	fn clear_spread_evaluates_to_transparency_outside_the_unit_range() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		assert_eq!(gradient.evaluate(-0.25, GradientSpread::Clear, Default::default(), Default::default()), Color::TRANSPARENT);
		assert_eq!(gradient.evaluate(1.25, GradientSpread::Clear, Default::default(), Default::default()), Color::TRANSPARENT);

		for t in [0., 0.25, 1.] {
			assert_eq!(
				gradient.evaluate(t, GradientSpread::Clear, Default::default(), Default::default()),
				gradient.evaluate(t, GradientSpread::Pad, Default::default(), Default::default()),
				"inside the range Clear must match Pad at t = {t}"
			);
		}
	}

	#[test]
	fn evaluate_follows_the_interpolation_space() {
		let gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);

		let oklab = gradient.evaluate(0.5, Default::default(), GradientInterpolation::OkLab, Default::default());
		let linear = gradient.evaluate(0.5, Default::default(), GradientInterpolation::SrgbLinear, Default::default());
		let gamma = gradient.evaluate(0.5, Default::default(), GradientInterpolation::SrgbGamma, Default::default());

		assert_eq!(linear, Color::BLACK.lerp(&Color::WHITE, 0.5));
		assert_eq!(gamma, Color::BLACK.lerp_gamma_srgb(&Color::WHITE, 0.5));

		// Halfway in OkLab between black and white is lightness 0.5, which cubes to 1/8 linear light in every channel
		for channel in [oklab.r(), oklab.g(), oklab.b()] {
			assert!((channel - 0.125).abs() < 1e-3, "the OkLab mid color of black and white should be 1/8 linear light, got {channel}");
		}
		assert_eq!(oklab.a(), 1.);

		assert_ne!(linear, gamma, "each space must produce a different mid color between black and white");
		assert_ne!(oklab, linear, "each space must produce a different mid color between black and white");
		assert_ne!(oklab, gamma, "each space must produce a different mid color between black and white");
	}

	#[test]
	fn polar_spaces_take_the_shorter_hue_arc_and_ignore_powerless_hues() {
		use color::ColorSpace;

		// Red to blue in HSL crosses through magenta on the shorter arc (300 degrees), not through green (120 degrees)
		let red_to_blue = Gradient::from(vec![Color::RED, Color::BLUE]);
		let magenta = red_to_blue.evaluate(0.5, Default::default(), GradientInterpolation::Hsl, Default::default());
		for (channel, expected) in [(magenta.r(), 1.), (magenta.g(), 0.), (magenta.b(), 1.)] {
			assert!((channel - expected).abs() < 1e-3, "the HSL mid color of red and blue should be magenta, got {magenta:?}");
		}

		// White's hue is powerless, so an OkLch blend toward it keeps red's hue instead of drifting toward white's arbitrary hue
		let red_to_white = Gradient::from(vec![Color::RED, Color::WHITE]);
		let pink = red_to_white.evaluate(0.5, Default::default(), GradientInterpolation::OkLch, Default::default());
		let [_, _, red_hue] = color::Oklch::from_linear_srgb([Color::RED.r(), Color::RED.g(), Color::RED.b()]);
		let [_, pink_chroma, pink_hue] = color::Oklch::from_linear_srgb([pink.r(), pink.g(), pink.b()]);
		assert!(pink_chroma > 0.05, "the mid color should stay chromatic, got {pink:?}");
		assert!((pink_hue - red_hue).abs() < 0.5, "the mid hue should hold red's {red_hue} degrees, got {pink_hue}");
	}

	#[test]
	fn hue_direction_chooses_the_arc_around_the_hue_wheel() {
		// Red to blue spans 240 degrees upward, so Longer and Increasing agree on the green route
		// while Shorter and Decreasing cross through magenta
		let red_to_blue = Gradient::from(vec![Color::RED, Color::BLUE]);
		let expectations = [
			(GradientHueDirection::Shorter, [1., 0., 1.]),
			(GradientHueDirection::Longer, [0., 1., 0.]),
			(GradientHueDirection::Increasing, [0., 1., 0.]),
			(GradientHueDirection::Decreasing, [1., 0., 1.]),
		];
		for (gradient_hue_direction, expected_rgb) in expectations {
			let mid = red_to_blue.evaluate(0.5, Default::default(), GradientInterpolation::Hsl, gradient_hue_direction);
			for (channel, target) in [mid.r(), mid.g(), mid.b()].into_iter().zip(expected_rgb) {
				assert!(
					(channel - target).abs() < 1e-3,
					"the {gradient_hue_direction:?} mid of red and blue should be {expected_rgb:?}, got {mid:?}"
				);
			}
		}

		// Identical hues under Longer take a full turn around the wheel, passing through cyan halfway
		let red_to_red = Gradient::from(vec![Color::RED, Color::RED]);
		let mid = red_to_red.evaluate(0.5, Default::default(), GradientInterpolation::Hsl, GradientHueDirection::Longer);
		for (channel, target) in [mid.r(), mid.g(), mid.b()].into_iter().zip([0., 1., 1.]) {
			assert!((channel - target).abs() < 1e-3, "the full-turn mid of red and red should be cyan, got {mid:?}");
		}
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

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default())
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert!(sample_positions.windows(2).all(|pair| pair[0] <= pair[1]), "samples must ascend: {sample_positions:?}");
		assert_eq!(sample_positions.first(), Some(&0.));
		assert_eq!(sample_positions.last(), Some(&1.));

		assert_eq!(gradient.evaluate(0., Default::default(), Default::default(), Default::default()), Color::RED);
		assert_eq!(gradient.evaluate(1., Default::default(), Default::default(), Default::default()), Color::WHITE);
	}

	#[test]
	fn infinite_positions_clamp_to_the_range_ends() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[f64::INFINITY, f64::NEG_INFINITY]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default())
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(gradient.evaluate(0., Default::default(), Default::default(), Default::default()), Color::BLACK);
		assert_eq!(gradient.evaluate(1., Default::default(), Default::default(), Default::default()), Color::WHITE);
	}

	#[test]
	fn nan_positions_drop_their_stops_from_sampling() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK, Color::RED]);
		gradient.set_positions(&[0., f64::NAN, 1.]);

		let sample_positions: Vec<f64> = gradient
			.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default())
			.iter()
			.map(|(position, ..)| *position)
			.collect();
		assert_eq!(sample_positions, vec![0., 1.]);
		assert_eq!(
			gradient.evaluate(0.5, Default::default(), GradientInterpolation::SrgbLinear, Default::default()),
			Color::WHITE.lerp(&Color::RED, 0.5)
		);

		// A non-finite position is preserved as nondefault so write-back elision cannot resurrect the dropped stop
		assert!(gradient.nondefault_positions().is_some());

		// With every position NaN the gradient samples as stopless, painting solid black to signal the upstream bug
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::RED]);
		gradient.set_positions(&[f64::NAN, f64::NAN]);
		assert!(gradient.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default()).is_empty());
		assert_eq!(gradient.evaluate(0.5, Default::default(), Default::default(), Default::default()), Color::BLACK);
	}

	#[test]
	fn samples_start_at_the_first_stop_without_synthetic_lead_in() {
		let mut gradient = Gradient::from(vec![Color::WHITE, Color::BLACK]);
		gradient.set_positions(&[0.3, 1.]);

		let samples = gradient.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default());
		assert_eq!(samples[0], (0.3, Color::WHITE, None), "renderers that need a flat lead-in before the first stop add it themselves");
	}

	#[test]
	fn nan_midpoints_read_as_linear() {
		let mut gradient = Gradient::from(vec![Color::BLACK, Color::WHITE]);
		let linear_result = gradient.evaluate(0.25, Default::default(), Default::default(), Default::default());

		gradient.set_midpoints(&[f64::NAN, f64::NAN]);
		assert_eq!(gradient.evaluate(0.25, Default::default(), Default::default(), Default::default()), linear_result);
		let no_nan_annotations = gradient
			.interpolated_samples(GradientInterpolation::SrgbGamma, Default::default())
			.iter()
			.all(|(position, _, midpoint)| position.is_finite() && !midpoint.is_some_and(|midpoint| midpoint.is_nan()));
		assert!(no_nan_annotations, "NaN must not escape into rendered sample annotations");
	}
}
