use crate::{affine, color};
use core_types::list::{ATTR_POSITION, Item};
use core_types::{ATTR_GRADIENT_FORM, ATTR_GRADIENT_SPACE, ATTR_GRADIENT_SPREAD, ATTR_TRANSFORM};
use glam::{DAffine2, DVec2};
use graphic_types::Graphic;
use graphic_types::vector_types::{Gradient, GradientForm, GradientSpace, GradientSpread};

pub fn linear(gradient: &usvg::LinearGradient, opacity: usvg::Opacity) -> Graphic {
	let start = DVec2::new(gradient.x1().into(), gradient.y1().into());
	let direction = DVec2::new(gradient.x2().into(), gradient.y2().into()) - start;
	if direction == DVec2::ZERO
		&& let Some(last) = gradient.stops().last()
	{
		return Graphic::from(color(last.color(), last.opacity().get() * opacity.get()));
	}
	paint(gradient, GradientForm::Linear, DAffine2::from_cols(direction, direction.perp(), start), opacity)
}

pub fn radial(gradient: &usvg::RadialGradient, opacity: usvg::Opacity) -> Graphic {
	let radius = DVec2::splat(gradient.r().get().into());
	let center = DVec2::new(gradient.cx().into(), gradient.cy().into());
	paint(gradient, GradientForm::Radial, DAffine2::from_scale_angle_translation(radius, 0., center), opacity)
}

fn paint(gradient: &usvg::BaseGradient, form: GradientForm, unit_placement: DAffine2, opacity: usvg::Opacity) -> Graphic {
	let stops = gradient
		.stops()
		.iter()
		.map(|stop| Item::new_from_element(color(stop.color(), stop.opacity().get() * opacity.get())).with_attribute(ATTR_POSITION, f64::from(stop.offset().get())))
		.collect();
	let mut item = Item::new_from_element(Gradient(stops)).with_attribute(ATTR_GRADIENT_SPACE, GradientSpace::RgbGamma);

	if form != GradientForm::default() {
		item.set_attribute(ATTR_GRADIENT_FORM, form);
	}
	let spread = match gradient.spread_method() {
		usvg::SpreadMethod::Pad => GradientSpread::Pad,
		usvg::SpreadMethod::Reflect => GradientSpread::Reflect,
		usvg::SpreadMethod::Repeat => GradientSpread::Repeat,
	};
	if !spread.is_default() {
		item.set_attribute(ATTR_GRADIENT_SPREAD, spread);
	}
	let transform = affine(gradient.transform()) * unit_placement;
	if transform != DAffine2::IDENTITY {
		item.set_attribute(ATTR_TRANSFORM, transform);
	}

	Graphic::Gradient(item)
}
