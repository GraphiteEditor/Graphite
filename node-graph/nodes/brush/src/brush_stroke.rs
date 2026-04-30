use core_types::CacheHash;
use core_types::blending::BlendMode;
use core_types::color::Color;
use core_types::math::bbox::AxisAlignedBbox;
use dyn_any::DynAny;
use glam::DVec2;
use std::hash::{Hash, Hasher};

/// The style of a brush.
#[derive(Clone, Debug, CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStyle {
	pub color: Color,
	pub diameter: f64,
	pub hardness: f64,
	pub flow: f64,
	pub spacing: f64, // Spacing as a fraction of the diameter.
	pub blend_mode: BlendMode,
}

impl Default for BrushStyle {
	fn default() -> Self {
		Self {
			color: Color::BLACK,
			diameter: 40.,
			hardness: 50.,
			flow: 100.,
			spacing: 50., // Percentage of diameter.
			blend_mode: BlendMode::Normal,
		}
	}
}

impl Eq for BrushStyle {}

impl PartialEq for BrushStyle {
	fn eq(&self, other: &Self) -> bool {
		self.color == other.color
			&& self.diameter.to_bits() == other.diameter.to_bits()
			&& self.hardness.to_bits() == other.hardness.to_bits()
			&& self.flow.to_bits() == other.flow.to_bits()
			&& self.spacing.to_bits() == other.spacing.to_bits()
			&& self.blend_mode == other.blend_mode
	}
}

/// A single sample of brush parameters across the brush stroke.
#[derive(Clone, Debug, PartialEq, core_types::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushInputSample {
	pub position: DVec2,
	pub pressure: f64,
	// Future work: stylus angle, etc.
}

impl Hash for BrushInputSample {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.position.x.to_bits().hash(state);
		self.position.y.to_bits().hash(state);
		self.pressure.to_bits().hash(state);
	}
}

/// Samples of blit point parameters along the brush stroke trace path.
#[derive(Clone, Debug, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct BrushOutputSample {
	// The position of the sample in layer space, in pixels.
	// The origin of layer space is not specified.
	pub position: DVec2,

	// The scale multiplier for the brush stamp diameter
	pub scale: f64,
	// Future work: stylus angle, etc.
}

/// The parameters for a single stroke brush.
#[derive(Clone, Debug, PartialEq, core_types::CacheHash, Default, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStroke {
	pub style: BrushStyle,
	pub trace: Vec<BrushInputSample>,
}

impl BrushStroke {
	pub fn bounding_box(&self) -> AxisAlignedBbox {
		let radius = self.style.diameter / 2.;
		self.compute_blit_points()
			.iter()
			.map(|sample| AxisAlignedBbox {
				start: sample.position + DVec2::new(-radius, -radius),
				end: sample.position + DVec2::new(radius, radius),
			})
			.reduce(|a, b| a.union(&b))
			.unwrap_or(AxisAlignedBbox::ZERO)
	}

	pub fn compute_blit_points(&self) -> Vec<BrushOutputSample> {
		// We always travel in a straight line towards the next user input,
		// placing a blit point every time we travelled our spacing distance.
		let spacing_distance = (self.style.spacing / 100.) * self.style.diameter;
		if self.trace.is_empty() {
			return Vec::new();
		};

		let mut current_position = self.trace[0].position;
		let mut current_pressure = self.trace[0].pressure;
		let mut result = vec![BrushOutputSample { position: current_position, scale: current_pressure, }];

		// We iterate over all input points and take uniform steps of length equal to our spacing distance across the entire stroke
		for sample in &self.trace[1..] {
			let position_delta = (sample.position - current_position).length();
			let pressure_delta = sample.pressure - current_pressure;

			// Skip input sample pairs with negligible position and pressure differences.
			if position_delta < f64::EPSILON && pressure_delta < f64::EPSILON {
				continue;
			}

			let spacing_step = (sample.position - current_position).normalize() * spacing_distance;
			let mut space_remaining = position_delta;
			while space_remaining > spacing_distance {
				current_position += spacing_step;
				current_pressure = sample.pressure - (space_remaining / position_delta) * pressure_delta;
				result.push(BrushOutputSample { position: current_position, scale: current_pressure });

				space_remaining -= spacing_distance;
			}
		}

		result
	}
}
