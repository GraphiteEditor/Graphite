use crate::color;
use core_types::list::{Item, List};
use core_types::transfer_curve::TransferCurve;
use core_types::{ATTR_EFFECTS, BlendMode};
use graphic_types::{EffectOperation, Effects, Graphic};
use usvg::filter::{ColorInterpolation, Input, Kind, Primitive};

pub fn filtered(group: &usvg::Group, content: List<Graphic>) -> List<Graphic> {
	let operations = (group.filters().iter())
		.filter_map(|filter| operation(filter.primitives()))
		.filter(|operation| !operation.changes_nothing());
	let effects = Effects::from_operations(operations);
	if effects.is_empty() {
		return content;
	}

	List::new_from_item(Item::new_from_element(Graphic::GraphicList(content)).with_attribute(ATTR_EFFECTS, effects))
}

fn operation(primitives: &[Primitive]) -> Option<EffectOperation> {
	let (primitive, rest) = primitives.split_first()?;
	let passes_through = |next: &Primitive| match next.kind() {
		Kind::Offset(offset) => offset.dx() == 0. && offset.dy() == 0. && matches!(offset.input(), Input::Reference(name) if name == primitive.result()),
		_ => false,
	};
	if rest.len() > 1 || !rest.iter().all(passes_through) {
		return None;
	}

	match primitive.kind() {
		Kind::GaussianBlur(blur) if *blur.input() == Input::SourceGraphic => Some(EffectOperation::Blur {
			radius_mask: Graphic::default(),
			radius: standard_deviation(blur.std_dev_x().get(), blur.std_dev_y().get()),
			box_blur: false,
			gamma: primitive.color_interpolation() == ColorInterpolation::SRGB,
		}),
		Kind::DropShadow(shadow) if *shadow.input() == Input::SourceGraphic => {
			let (dx, dy) = (f64::from(shadow.dx()), f64::from(shadow.dy()));
			let tent = standard_deviation(shadow.std_dev_x().get(), shadow.std_dev_y().get()) * 6_f64.sqrt();
			Some(EffectOperation::DropShadow {
				color: color(shadow.color(), shadow.opacity().get()),
				// SVG composites a drop shadow beneath its source normally
				blend_mode: BlendMode::Normal,
				angle: dy.atan2(-dx).to_degrees(),
				distance: dx.hypot(dy),
				spread: 0.,
				size: if tent >= 2. { tent } else { tent / 2. },
				contour: TransferCurve::default(),
				knockout: true,
			})
		}
		_ => None,
	}
}

fn standard_deviation(along_x: f32, along_y: f32) -> f64 {
	(f64::from(along_x) * f64::from(along_y)).sqrt()
}
