use core_types::BlendMode;

pub fn blend_mode(group: &usvg::Group) -> Option<BlendMode> {
	match group.blend_mode() {
		usvg::BlendMode::Normal | usvg::BlendMode::PlusLighter => None,
		usvg::BlendMode::Multiply => Some(BlendMode::Multiply),
		usvg::BlendMode::Screen => Some(BlendMode::Screen),
		usvg::BlendMode::Overlay => Some(BlendMode::Overlay),
		usvg::BlendMode::Darken => Some(BlendMode::Darken),
		usvg::BlendMode::Lighten => Some(BlendMode::Lighten),
		usvg::BlendMode::ColorDodge => Some(BlendMode::ColorDodge),
		usvg::BlendMode::ColorBurn => Some(BlendMode::ColorBurn),
		usvg::BlendMode::HardLight => Some(BlendMode::HardLight),
		usvg::BlendMode::SoftLight => Some(BlendMode::SoftLight),
		usvg::BlendMode::Difference => Some(BlendMode::Difference),
		usvg::BlendMode::Exclusion => Some(BlendMode::Exclusion),
		usvg::BlendMode::Hue => Some(BlendMode::Hue),
		usvg::BlendMode::Saturation => Some(BlendMode::Saturation),
		usvg::BlendMode::Color => Some(BlendMode::Color),
		usvg::BlendMode::Luminosity => Some(BlendMode::Luminosity),
	}
}
