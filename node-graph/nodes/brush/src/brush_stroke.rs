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
		let spacing_dist = self.style.spacing / 100. * self.style.diameter;
		if self.trace.is_empty() {
			return Vec::new();
		};

		let mut result = vec![BrushOutputSample {
			position: self.trace[0].position,
			scale: self.trace[0].pressure,
		}];

		for samples in self.trace.windows(2) {
			let position_delta = (samples[1].position - samples[0].position).length();
			let unit_step = (samples[1].position - samples[0].position).normalize();
			let pressure_delta = samples[1].pressure - samples[0].pressure;
			let mut current_position = samples[0].position + unit_step;
			loop {
				let step = (samples[1].position - current_position).length();
				if step < spacing_dist {
					break;
				}
				// let t64 = t as f64;
				result.push(BrushOutputSample {
					position: current_position,
					scale: samples[1].pressure - (step / position_delta) * pressure_delta,
				});
				current_position += unit_step;
			}

			result.push(BrushOutputSample {
				position: samples[1].position,
				scale: samples[1].pressure,
			});
		}

		result
	}
}
