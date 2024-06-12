use dyn_any::{DynAny, StaticType};

use super::keyframe::KeyframeF64;

#[derive(Debug, Copy, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Easing {
	Linear,
}

impl Easing {
	pub fn interpolate(k1: &KeyframeF64, k2: &KeyframeF64, time: f64) -> f64 {
		match k1.easing {
			Easing::Linear => {
				assert!(k1.time < time && time < k2.time);
				let t = (time - k1.time) / (k2.time - k1.time);
				k1.value + (k2.value - k1.value) * t
			}
		}
	}
}
