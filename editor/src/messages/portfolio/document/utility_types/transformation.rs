use crate::consts::{ROTATE_SNAP_ANGLE, SCALE_SNAP_INTERVAL};
use crate::messages::portfolio::document::node_graph::VectorDataModification;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::ToolType;
use document_legacy::document::Document;
use document_legacy::layers::style::RenderData;
use document_legacy::LayerId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::{DAffine2, DVec2};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, PartialEq, Clone)]
pub enum OriginalTransforms {
	Layer(HashMap<Vec<LayerId>, DAffine2>),
	Path(HashMap<Vec<LayerId>, Vec<(ManipulatorPointId, DVec2)>>),
}
impl Default for OriginalTransforms {
	fn default() -> Self {
		OriginalTransforms::Path(HashMap::new())
	}
}
impl OriginalTransforms {
	pub fn clear(&mut self) {
		match self {
			OriginalTransforms::Layer(layer_map) => layer_map.clear(),
			OriginalTransforms::Path(path_map) => path_map.clear(),
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Copy)]
pub enum Axis {
	#[default]
	Both,
	X,
	Y,
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
	pub fn set_amount(self, change: DVec2) -> Self {
		Self {
			dragged_distance: change,
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
	pub fn set_amount(self, angle: f64) -> Self {
		Self {
			dragged_angle: angle,
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

	pub fn set_amount(self, change: f64) -> Self {
		Self {
			dragged_factor: 1. + change,
			typed_factor: None,
			constraint: self.constraint,
		}
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
pub enum TransformOperation {
	#[default]
	None,
	Grabbing(Translation),
	Rotating(Rotation),
	Scaling(Scale),
}

impl TransformOperation {
	pub fn apply_transform_operation(&self, selected: &mut Selected, snapping: bool, axis_constraint: Axis, grid: bool, mouse_movement: Option<DVec2>) {
		if self != &TransformOperation::None {
			let transformation = match self {
				TransformOperation::Grabbing(translation) => DAffine2::from_translation(translation.to_dvec()),
				TransformOperation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64(snapping)),
				TransformOperation::Scaling(scale) => DAffine2::from_scale(scale.to_dvec(snapping)),
				TransformOperation::None => unreachable!(),
			};

			selected.update_transforms(transformation, grid, Some(*self), mouse_movement);
			self.hints(snapping, axis_constraint, selected.responses);
		}
	}

	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected, snapping: bool, grid: bool) {
		match self {
			TransformOperation::None => (),
			TransformOperation::Grabbing(translation) => translation.constraint.set_or_toggle(axis),
			TransformOperation::Rotating(_) => (),
			TransformOperation::Scaling(scale) => scale.constraint.set_or_toggle(axis),
		};
		self.apply_transform_operation(selected, snapping, axis, grid, None);
	}

	pub fn grs_typed(&mut self, typed: Option<f64>, selected: &mut Selected, snapping: bool, grid: bool) {
		match self {
			TransformOperation::None => (),
			TransformOperation::Grabbing(translation) => translation.typed_distance = typed,
			TransformOperation::Rotating(rotation) => rotation.typed_angle = typed,
			TransformOperation::Scaling(scale) => scale.typed_factor = typed,
		};

		let axis_constraint = match self {
			TransformOperation::Grabbing(grabbing) => grabbing.constraint,
			TransformOperation::Scaling(scaling) => scaling.constraint,
			_ => Axis::Both,
		};
		self.apply_transform_operation(selected, snapping, axis_constraint, grid, None);
	}

	pub fn hints(&self, snapping: bool, axis_constraint: Axis, responses: &mut VecDeque<Message>) {
		use crate::messages::input_mapper::utility_types::input_keyboard::Key;
		use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

		let mut hints = Vec::new();

		let axis_str = |vector: DVec2, separate: bool| match axis_constraint {
			Axis::Both => {
				if separate {
					format!("X: {}, Y: {}", vector.x, vector.y)
				} else {
					vector.x.to_string()
				}
			}
			Axis::X => format!("X: {}", vector.x),
			Axis::Y => format!("Y: {}", vector.y),
		};

		let value_str = match self {
			TransformOperation::None => String::new(),
			TransformOperation::Grabbing(translation) => format!("Translate {}", axis_str(translation.to_dvec(), true)),
			TransformOperation::Rotating(rotation) => format!("Rotate {}°", rotation.to_f64(snapping) * 360. / std::f64::consts::TAU),
			TransformOperation::Scaling(scale) => format!("Scale {}", axis_str(scale.to_dvec(snapping), false)),
		};
		hints.push(HintInfo::label(value_str));
		hints.push(HintInfo::keys([Key::Shift], "Precision Mode"));
		if matches!(self, TransformOperation::Rotating(_) | TransformOperation::Scaling(_)) {
			hints.push(HintInfo::keys([Key::Control], "Snap"));
		}
		if matches!(self, TransformOperation::Grabbing(_) | TransformOperation::Scaling(_)) {
			hints.push(HintInfo::keys([Key::KeyX], "X Axis"));
			hints.push(HintInfo::keys([Key::KeyY], "Y Axis"));
		}

		let hint_data = HintData(vec![HintGroup(hints)]);
		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}
}

pub struct Selected<'a> {
	pub selected: &'a [&'a Vec<LayerId>],
	pub responses: &'a mut VecDeque<Message>,
	pub document: &'a Document,
	pub original_transforms: &'a mut OriginalTransforms,
	pub pivot: &'a mut DVec2,
	pub shape_editor: Option<&'a ShapeState>,
	pub tool_type: &'a ToolType,
}

impl<'a> Selected<'a> {
	pub fn new(
		original_transforms: &'a mut OriginalTransforms,
		pivot: &'a mut DVec2,
		selected: &'a [&'a Vec<LayerId>],
		responses: &'a mut VecDeque<Message>,
		document: &'a Document,
		shape_editor: Option<&'a ShapeState>,
		tool_type: &'a ToolType,
	) -> Self {
		// If user is using the Select tool then use the original layer transforms
		if (*tool_type == ToolType::Select) && (*original_transforms == OriginalTransforms::Path(HashMap::new())) {
			*original_transforms = OriginalTransforms::Layer(HashMap::new());
		}

		match original_transforms {
			OriginalTransforms::Layer(layer_map) => {
				for layer_path in selected {
					if !layer_map.contains_key(*layer_path) {
						if let Ok(layer) = document.layer(layer_path) {
							layer_map.insert(layer_path.to_vec(), layer.transform);
						} else {
							warn!("Didn't find a layer for {:?}", layer_path);
						}
					}
				}
			}
			OriginalTransforms::Path(path_map) => {
				for path in selected {
					let Some(shape_editor) = shape_editor else {
						warn!("No shape editor structure found, which only happens in select tool, which cannot reach this point as we check for ToolType");
						continue;
					};
					// Anchors also move their handles
					let expand_anchors = |&point: &ManipulatorPointId| {
						if point.manipulator_type.is_handle() {
							[Some(point), None, None]
						} else {
							[
								Some(point),
								Some(ManipulatorPointId::new(point.group, SelectedType::InHandle)),
								Some(ManipulatorPointId::new(point.group, SelectedType::OutHandle)),
							]
						}
					};
					let points = shape_editor.selected_points().flat_map(expand_anchors).flatten();
					if path_map.contains_key(*path) {
						continue;
					}
					let Ok(layer) = document.layer(path) else {
						warn!("Didn't find a layer for {:?}", path);
						continue;
					};
					let Some(vector_data) = layer.as_vector_data() else {
						continue;
					};
					let get_manipulator_point_position = |point_id: ManipulatorPointId| {
						vector_data
							.manipulator_from_id(point_id.group)
							.and_then(|manipulator_group| point_id.manipulator_type.get_position(manipulator_group))
							.map(|position| (point_id, position))
					};
					path_map.insert(path.to_vec(), points.filter_map(get_manipulator_point_position).collect());
				}
			}
		}
		Self {
			selected,
			responses,
			document,
			original_transforms,
			pivot,
			shape_editor,
			tool_type,
		}
	}

	pub fn mean_average_of_pivots(&mut self, render_data: &RenderData) -> DVec2 {
		let xy_summation = self.selected.iter().filter_map(|path| self.document.pivot(path, render_data)).reduce(|a, b| a + b).unwrap_or_default();

		xy_summation / self.selected.len() as f64
	}

	pub fn center_of_aabb(&mut self, render_data: &RenderData) -> DVec2 {
		let [min, max] = self
			.selected
			.iter()
			.filter_map(|path| {
				let multiplied_transform = self.document.multiply_transforms(path).unwrap();

				self.document.layer(path).unwrap().aabb_for_transform(multiplied_transform, render_data)
			})
			.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
			.unwrap_or_default();
		(min + max) / 2.
	}

	pub fn update_transforms(&mut self, delta: DAffine2, grid: bool, transform_operator: Option<TransformOperation>, mouse_movement: Option<DVec2>) {
		if !self.selected.is_empty() {
			let doc_transform = self.document.root.transform;
			let pivot_point = doc_transform.transform_point2(*self.pivot);

			let pivot = DAffine2::from_translation(pivot_point);
			let transformation = pivot * delta * pivot.inverse();

			// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
			for layer_path in Document::shallowest_unique_layers(self.selected.iter()) {
				let parent_folder_path = &layer_path[..layer_path.len() - 1];
				let viewspace = self.document.generate_transform_relative_to_viewport(layer_path).ok().unwrap_or_default();

				if *self.tool_type == ToolType::Select {
					let original_layer_transforms = match self.original_transforms {
						OriginalTransforms::Layer(layer_map) => *layer_map.get(*layer_path).unwrap(),
						OriginalTransforms::Path(_path_map) => {
							warn!("Found Path variant in original_transforms, returning identity transform for layer {:?}", layer_path);
							DAffine2::IDENTITY
						}
					};

					let to = self.document.generate_transform_across_scope(parent_folder_path, None).unwrap();
					let mut new = to.inverse() * transformation * to * original_layer_transforms;

					match transform_operator {
						Some(transform_operation) => {
							if let TransformOperation::Grabbing(_) = transform_operator.unwrap_or_default() {
								match mouse_movement {
									Some(direction) => {
										if grid {
											// Find the current position in doc space
											let viewspace_pos = viewspace.transform_point2(DVec2 { x: 0.0, y: 0.0 });
											let doc_pos = self.document.root.transform.inverse().transform_point2(viewspace_pos);

											// If there is movement of the mouse
											if direction.x > 0.0 || direction.x < 0.0 || direction.y > 0.0 || direction.y < 0.0 {
												// Update the translation by rounding the document position based on the direction
												let mut x_changed = false;
												let mut y_changed = false;
												if direction.x > 0.0 {
													x_changed = true;
													new.translation.x = new.translation.x.ceil();
												} else if direction.x < -0.0 {
													x_changed = true;
													new.translation.x = new.translation.x.floor();
												}
												if direction.y > 0.0 {
													y_changed = true;
													new.translation.y = new.translation.y.ceil();
												} else if direction.y < -0.0 {
													y_changed = true;
													new.translation.y = new.translation.y.floor();
												}
												// If the x or y was unchanged, use the original x or y position
												if !x_changed {
													new.translation.x = doc_pos.x;
												}
												if !y_changed {
													new.translation.y = doc_pos.y;
												}

												self.responses.add(GraphOperationMessage::TransformSet {
													layer: layer_path.to_vec(),
													transform: new,
													transform_in: TransformIn::Local,
													skip_rerender: true,
												});
											}
										} else if !grid {
											self.responses.add(GraphOperationMessage::TransformSet {
												layer: layer_path.to_vec(),
												transform: new,
												transform_in: TransformIn::Local,
												skip_rerender: true,
											});
										}
									}
									None => {}
								}
							}

							if let TransformOperation::Rotating(_) = transform_operator.unwrap_or_default() {
								self.responses.add(GraphOperationMessage::TransformSet {
									layer: layer_path.to_vec(),
									transform: new,
									transform_in: TransformIn::Local,
									skip_rerender: true,
								});
							}

							if let TransformOperation::Scaling(_) = transform_operator.unwrap_or_default() {
								self.responses.add(GraphOperationMessage::TransformSet {
									layer: layer_path.to_vec(),
									transform: new,
									transform_in: TransformIn::Local,
									skip_rerender: true,
								});
							}
						}

						None => {
							self.responses.add(GraphOperationMessage::TransformSet {
								layer: layer_path.to_vec(),
								transform: new,
								transform_in: TransformIn::Local,
								skip_rerender: true,
							});
						}
					}
				}

				if *self.tool_type == ToolType::Path {
					let layerspace_rotation = viewspace.inverse() * transformation;
					let initial_points = match self.original_transforms {
						OriginalTransforms::Layer(_layer_map) => {
							warn!("Found Layer variant in original_transforms when Path wanted, returning identity transform for layer");
							None
						}
						OriginalTransforms::Path(path_map) => path_map.get(*layer_path),
					};

					let Some(original) = initial_points else {
						warn!("Initial Points empty, it should not be possible to reach here without points");
						continue;
					};
					for (point_id, position) in original {
						//layerspace positon to viewport
						let viewport_point = viewspace.transform_point2(*position);

						//apply transformation and convert from viewport to layerspace position
						let layer_spacepos = layerspace_rotation.transform_point2(viewport_point);
						let position: DVec2 = layer_spacepos;
						let point = *point_id;

						self.responses.add(GraphOperationMessage::Vector {
							layer: (*layer_path).to_vec(),
							modification: VectorDataModification::SetManipulatorPosition { point, position },
						});
					}
				}
				self.responses.add(BroadcastEvent::DocumentIsDirty);
			}
		}
	}

	pub fn revert_operation(&mut self) {
		for path in self.selected.iter().copied() {
			let original_transform = &self.original_transforms;
			match original_transform {
				OriginalTransforms::Layer(hash) => {
					let Some(matrix) = hash.get(path) else { continue };
					self.responses.add(GraphOperationMessage::TransformSet {
						layer: path.to_vec(),
						transform: *matrix,
						transform_in: TransformIn::Local,
						skip_rerender: false,
					});
				}
				OriginalTransforms::Path(path) => {
					for (layer_path, points) in path {
						for &(point, position) in points {
							self.responses.add(GraphOperationMessage::Vector {
								layer: (*layer_path).clone(),
								modification: VectorDataModification::SetManipulatorPosition { point, position },
							});
						}
					}
				}
			}
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
