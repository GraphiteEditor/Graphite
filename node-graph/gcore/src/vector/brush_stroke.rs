use crate::raster::bbox::AxisAlignedBbox;
use crate::Color;

use dyn_any::{DynAny, StaticType};
use glam::DVec2;
use std::hash::{Hash, Hasher};

/// The style of a brush.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStyle {
	pub color: Color,
	pub diameter: f64,
	pub hardness: f64,
	pub flow: f64,
	pub spacing: f64, // Spacing as a fraction of the diameter.
}

impl Default for BrushStyle {
	fn default() -> Self {
		Self {
			color: Color::BLACK,
			diameter: 40.,
			hardness: 50.,
			flow: 100.,
			spacing: 50., // Percentage of diameter.
		}
	}
}

impl Hash for BrushStyle {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.color.hash(state);
		self.diameter.to_bits().hash(state);
		self.hardness.to_bits().hash(state);
		self.flow.to_bits().hash(state);
	}
}

/// A single sample of brush parameters across the brush stroke.
#[derive(Clone, Debug, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushInputSample {
	pub position: DVec2,
	// Future work: pressure, stylus angle, etc.
}

impl Hash for BrushInputSample {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.position.x.to_bits().hash(state);
		self.position.y.to_bits().hash(state);
	}
}

/// The parameters for a single stroke brush.
#[derive(Clone, Debug, PartialEq, Hash, Default, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushStroke {
	pub style: BrushStyle,
	pub trace: Vec<BrushInputSample>,
}

impl BrushStroke {
	pub fn bounding_box(&self) -> AxisAlignedBbox {
		let radius = self.style.diameter / 2.;
		self.trace
			.iter()
			.map(|sample| AxisAlignedBbox {
				start: sample.position + DVec2::new(-radius, -radius),
				end: sample.position + DVec2::new(radius, radius),
			})
			.reduce(|a, b| a.union(&b))
			.unwrap_or(AxisAlignedBbox::ZERO)
	}

	pub fn compute_blit_points(&self) -> Vec<DVec2> {
		// We always travel in a straight line towards the next user input,
		// placing a blit point every time we travelled our spacing distance.
		let spacing_dist = self.style.spacing / 100. * self.style.diameter;

		let Some(first_sample) = self.trace.first() else { return Vec::new(); };

		let mut cur_pos = first_sample.position;
		let mut result = vec![cur_pos];
		let mut dist_until_next_blit = spacing_dist;
		for sample in &self.trace[1..] {
			// Travel to the next sample.
			let delta = sample.position - cur_pos;
			let mut dist_left = delta.length();
			let unit_step = delta / dist_left;

			while dist_left >= dist_until_next_blit {
				// Take a step to the next blit point.
				cur_pos += dist_until_next_blit * unit_step;
				dist_left -= dist_until_next_blit;

				// Blit.
				result.push(cur_pos);
				dist_until_next_blit = spacing_dist;
			}

			// Take the partial step to land at the sample.
			dist_until_next_blit -= dist_left;
			cur_pos = sample.position;
		}

		result
	}
}
