use crate::consts::{ROTATE_SNAP_ANGLE, SCALE_SNAP_INTERVAL};
use crate::message_prelude::*;

use graphene::document::Document;
use graphene::layers::text_layer::FontCache;
use graphene::Operation as DocumentOperation;

use glam::{DAffine2, DVec2};
use std::collections::{HashMap, VecDeque};

pub type OriginalTransforms = HashMap<Vec<LayerId>, DAffine2>;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Axis {
	Both,
	X,
	Y,
}

impl Default for Axis {
	fn default() -> Self {
		Self::Both
	}
}

impl Axis {
	pub fn set_or_toggle(&mut self, target: Axis) {
		// If constrained to an axis and target is requesting the same axis, toggle back to Both
		if *self == target {
			*self = Axis::Both;
		}
		// If current axis is different from the target axis, switch to the target
		else {
			*self = target;
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
pub struct Translation {
	pub dragged_distance: DVec2,
	pub typed_distance: Option<f64>,
	pub constraint: Axis,
}

impl Translation {
	pub fn to_dvec(self) -> DVec2 {
		if let Some(value) = self.typed_distance {
			if self.constraint == Axis::Y {
				return DVec2::new(0., value);
			} else {
				return DVec2::new(value, 0.);
			}
		}

		match self.constraint {
			Axis::Both => self.dragged_distance,
			Axis::X => DVec2::new(self.dragged_distance.x, 0.),
			Axis::Y => DVec2::new(0., self.dragged_distance.y),
		}
	}

	#[must_use]
	pub fn increment_amount(self, delta: DVec2) -> Self {
		Self {
			dragged_distance: self.dragged_distance + delta,
			typed_distance: None,
			constraint: self.constraint,
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
pub struct Rotation {
	pub dragged_angle: f64,
	pub typed_angle: Option<f64>,
}

impl Rotation {
	pub fn to_f64(self, snap: bool) -> f64 {
		if let Some(value) = self.typed_angle {
			value.to_radians()
		} else if snap {
			let snap_resolution = ROTATE_SNAP_ANGLE.to_radians();
			(self.dragged_angle / snap_resolution).round() * snap_resolution
		} else {
			self.dragged_angle
		}
	}

	#[must_use]
	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			dragged_angle: self.dragged_angle + delta,
			typed_angle: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct Scale {
	pub dragged_factor: f64,
	pub typed_factor: Option<f64>,
	pub constraint: Axis,
}

impl Default for Scale {
	fn default() -> Self {
		Self {
			dragged_factor: 1.,
			typed_factor: None,
			constraint: Axis::default(),
		}
	}
}

impl Scale {
	pub fn to_dvec(self, snap: bool) -> DVec2 {
		let factor = if let Some(value) = self.typed_factor { value } else { self.dragged_factor };
		let factor = if snap { (factor / SCALE_SNAP_INTERVAL).round() * SCALE_SNAP_INTERVAL } else { factor };

		match self.constraint {
			Axis::Both => DVec2::splat(factor),
			Axis::X => DVec2::new(factor, 1.),
			Axis::Y => DVec2::new(1., factor),
		}
	}

	#[must_use]
	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			dragged_factor: self.dragged_factor + delta,
			typed_factor: None,
			constraint: self.constraint,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum TransformOperation {
	None,
	Grabbing(Translation),
	Rotating(Rotation),
	Scaling(Scale),
}

impl Default for TransformOperation {
	fn default() -> Self {
		TransformOperation::None
	}
}

impl TransformOperation {
	pub fn apply_transform_operation(&self, selected: &mut Selected, snapping: bool) {
		if self != &TransformOperation::None {
			let transformation = match self {
				TransformOperation::Grabbing(translation) => DAffine2::from_translation(translation.to_dvec()),
				TransformOperation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64(snapping)),
				TransformOperation::Scaling(scale) => DAffine2::from_scale(scale.to_dvec(snapping)),
				TransformOperation::None => unreachable!(),
			};

			selected.update_transforms(transformation);
		}
	}

	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected, snapping: bool) {
		match self {
			TransformOperation::None => (),
			TransformOperation::Grabbing(translation) => translation.constraint.set_or_toggle(axis),
			TransformOperation::Rotating(_) => (),
			TransformOperation::Scaling(scale) => scale.constraint.set_or_toggle(axis),
		};

		self.apply_transform_operation(selected, snapping);
	}

	pub fn handle_typed(&mut self, typed: Option<f64>, selected: &mut Selected, snapping: bool) {
		match self {
			TransformOperation::None => (),
			TransformOperation::Grabbing(translation) => translation.typed_distance = typed,
			TransformOperation::Rotating(rotation) => rotation.typed_angle = typed,
			TransformOperation::Scaling(scale) => scale.typed_factor = typed,
		};

		self.apply_transform_operation(selected, snapping);
	}
}

pub struct Selected<'a> {
	pub selected: &'a [&'a Vec<LayerId>],
	pub responses: &'a mut VecDeque<Message>,
	pub document: &'a Document,
	pub original_transforms: &'a mut OriginalTransforms,
	pub pivot: &'a mut DVec2,
}

impl<'a> Selected<'a> {
	pub fn new(original_transforms: &'a mut OriginalTransforms, pivot: &'a mut DVec2, selected: &'a [&'a Vec<LayerId>], responses: &'a mut VecDeque<Message>, document: &'a Document) -> Self {
		for path in selected {
			if !original_transforms.contains_key(*path) {
				original_transforms.insert(path.to_vec(), document.layer(path).unwrap().transform);
			}
		}
		Self {
			selected,
			responses,
			document,
			original_transforms,
			pivot,
		}
	}

	pub fn calculate_pivot(&mut self, font_cache: &FontCache) -> DVec2 {
		let xy_summation = self
			.selected
			.iter()
			.map(|path| {
				let multiplied_transform = self.document.multiply_transforms(path).unwrap();

				let bounds = self
					.document
					.layer(path)
					.unwrap()
					.aabounding_box_for_transform(multiplied_transform, font_cache)
					.unwrap_or([multiplied_transform.translation; 2]);

				(bounds[0] + bounds[1]) / 2.
			})
			.fold(DVec2::ZERO, |summation, next| summation + next);

		xy_summation / self.selected.len() as f64
	}

	pub fn update_transforms(&mut self, delta: DAffine2) {
		if !self.selected.is_empty() {
			let pivot = DAffine2::from_translation(*self.pivot);
			let transformation = pivot * delta * pivot.inverse();

			// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
			for layer_path in Document::shallowest_unique_layers(self.selected.iter()) {
				let parent_folder_path = &layer_path[..layer_path.len() - 1];
				let original_layer_transforms = *self.original_transforms.get(*layer_path).unwrap();

				let to = self.document.generate_transform_across_scope(parent_folder_path, None).unwrap();
				let new = to.inverse() * transformation * to * original_layer_transforms;

				self.responses.push_back(
					DocumentOperation::SetLayerTransform {
						path: layer_path.to_vec(),
						transform: new.to_cols_array(),
					}
					.into(),
				);
			}

			self.responses.push_back(ToolMessage::DocumentIsDirty.into());
		}
	}

	pub fn revert_operation(&mut self) {
		for path in self.selected {
			self.responses.push_back(
				DocumentOperation::SetLayerTransform {
					path: path.to_vec(),
					transform: (*self.original_transforms.get(*path).unwrap()).to_cols_array(),
				}
				.into(),
			);
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Typing {
	pub digits: Vec<u8>,
	pub contains_decimal: bool,
	pub negative: bool,
}

const DECIMAL_POINT: u8 = 10;

impl Typing {
	pub fn type_number(&mut self, number: u8) -> Option<f64> {
		self.digits.push(number);

		self.evaluate()
	}

	pub fn type_backspace(&mut self) -> Option<f64> {
		if self.digits.is_empty() {
			return None;
		}

		match self.digits.pop() {
			Some(DECIMAL_POINT) => self.contains_decimal = false,
			Some(_) => (),
			None => self.negative = false,
		}

		self.evaluate()
	}

	pub fn type_decimal_point(&mut self) -> Option<f64> {
		if !self.contains_decimal {
			self.contains_decimal = true;
			self.digits.push(DECIMAL_POINT);
		}

		self.evaluate()
	}

	pub fn type_negate(&mut self) -> Option<f64> {
		self.negative = !self.negative;

		self.evaluate()
	}

	pub fn evaluate(&self) -> Option<f64> {
		if self.digits.is_empty() {
			return None;
		}

		let mut result = 0_f64;
		let mut running_decimal_place = 0_i32;

		for digit in &self.digits {
			if *digit == DECIMAL_POINT {
				if running_decimal_place == 0 {
					running_decimal_place = 1;
				}
			} else if running_decimal_place == 0 {
				result *= 10.;
				result += *digit as f64;
			} else {
				result += *digit as f64 * 0.1_f64.powi(running_decimal_place);
				running_decimal_place += 1;
			}
		}

		if self.negative {
			result = -result;
		}

		Some(result)
	}

	pub fn clear(&mut self) {
		self.digits.clear();
		self.contains_decimal = false;
		self.negative = false;
	}
}
