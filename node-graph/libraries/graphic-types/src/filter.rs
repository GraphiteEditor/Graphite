#[derive(Debug, Clone, PartialEq, graphene_hash::CacheHash, dyn_any::DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SvgFilterEffect {
	GaussianBlur {
		std_deviation_x: f64,
		std_deviation_y: f64,
	},
}

impl Default for SvgFilterEffect {
	fn default() -> Self {
		Self::GaussianBlur {
			std_deviation_x: 0.,
			std_deviation_y: 0.,
		}
	}
}
