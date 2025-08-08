use super::network_interface::NodeNetworkInterface;
use crate::consts::{ROTATE_INCREMENT, SCALE_INCREMENT};
use crate::messages::portfolio::document::graph_operation::transform_utils;
use crate::messages::portfolio::document::graph_operation::utility_types::{ModifyInputsContext, TransformIn};
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::ToolType;
use glam::{DAffine2, DMat2, DVec2};
use graphene_std::renderer::Quad;
use graphene_std::vector::misc::{HandleId, ManipulatorPointId};
use graphene_std::vector::{HandleExt, PointId, VectorModificationType};
use std::collections::{HashMap, VecDeque};
use std::f64::consts::PI;

#[derive(Debug, PartialEq, Clone, Copy)]
struct AnchorPoint {
	initial: DVec2,
	current: DVec2,
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct HandlePoint {
	initial: DVec2,
	relative: DVec2,
	anchor: PointId,
	mirror: Option<(HandleId, DVec2)>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct InitialPoints {
	anchors: HashMap<PointId, AnchorPoint>,
	handles: HashMap<HandleId, HandlePoint>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum OriginalTransforms {
	Layer(HashMap<LayerNodeIdentifier, DAffine2>),
	Path(HashMap<LayerNodeIdentifier, InitialPoints>),
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

	/// Gets the transform from the most downstream transform node
	fn get_layer_transform(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<DAffine2> {
		let transform_node_id = ModifyInputsContext::locate_node_in_layer_chain("Transform", layer, network_interface)?;

		let document_node = network_interface.document_network().nodes.get(&transform_node_id)?;
		Some(transform_utils::get_current_transform(&document_node.inputs))
	}

	pub fn update<'a>(&mut self, selected: &'a [LayerNodeIdentifier], network_interface: &NodeNetworkInterface, shape_editor: Option<&'a ShapeState>) {
		match self {
			OriginalTransforms::Layer(layer_map) => {
				layer_map.retain(|layer, _| selected.contains(layer));
				for &layer in selected {
					if layer == LayerNodeIdentifier::ROOT_PARENT {
						continue;
					}

					layer_map.entry(layer).or_insert_with(|| Self::get_layer_transform(layer, network_interface).unwrap_or_default());
				}
			}
			OriginalTransforms::Path(path_map) => {
				let Some(shape_editor) = shape_editor else {
					warn!("No shape editor structure found, which only happens in select tool, which cannot reach this point as we check for ToolType");
					return;
				};
				for &layer in selected {
					if path_map.contains_key(&layer) {
						continue;
					}
					let Some(vector) = network_interface.compute_modified_vector(layer) else {
						continue;
					};
					let Some(selected_points) = shape_editor.selected_points_in_layer(layer) else {
						continue;
					};
					let Some(selected_segments) = shape_editor.selected_segments_in_layer(layer) else {
						continue;
					};

					let mut selected_points = selected_points.clone();

					for (segment_id, _, start, end) in vector.segment_bezier_iter() {
						if selected_segments.contains(&segment_id) {
							selected_points.insert(ManipulatorPointId::Anchor(start));
							selected_points.insert(ManipulatorPointId::Anchor(end));
						}
					}

					// Anchors also move their handles
					let anchor_ids = selected_points.iter().filter_map(|point| point.as_anchor());
					let anchors = anchor_ids.filter_map(|id| vector.point_domain.position_from_id(id).map(|pos| (id, AnchorPoint { initial: pos, current: pos })));
					let anchors = anchors.collect();

					let selected_handles = selected_points.iter().filter_map(|point| point.as_handle());
					let anchor_ids = selected_points.iter().filter_map(|point| point.as_anchor());
					let connected_handles = anchor_ids.flat_map(|point| vector.all_connected(point));
					let all_handles = selected_handles.chain(connected_handles);

					let handles = all_handles
						.filter_map(|id| {
							let anchor = id.to_manipulator_point().get_anchor(&vector)?;
							let initial = id.to_manipulator_point().get_position(&vector)?;
							let relative = vector.point_domain.position_from_id(anchor)?;
							let other_handle = vector
								.other_colinear_handle(id)
								.filter(|other| !selected_points.contains(&other.to_manipulator_point()) && !selected_points.contains(&ManipulatorPointId::Anchor(anchor)));
							let mirror = other_handle.and_then(|id| Some((id, id.to_manipulator_point().get_position(&vector)?)));

							Some((id, HandlePoint { initial, relative, anchor, mirror }))
						})
						.collect();

					path_map.insert(layer, InitialPoints { anchors, handles });
				}
			}
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
	pub fn contrainted_to_axis(self, target: Axis, local: bool) -> (Self, bool) {
		if self != target {
			return (target, false);
		}

		if local { (Axis::Both, false) } else { (self, true) }
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
pub struct Translation {
	pub dragged_distance: DVec2,
	pub typed_distance: Option<f64>,
	pub constraint: Axis,
}

impl Translation {
	pub fn to_dvec(self, transform: DAffine2, increment_mode: bool) -> DVec2 {
		let displacement = if let Some(value) = self.typed_distance {
			match self.constraint {
				Axis::X => transform.transform_vector2(DVec2::new(value, 0.)),
				Axis::Y => transform.transform_vector2(DVec2::new(0., value)),
				Axis::Both => self.dragged_distance,
			}
		} else {
			match self.constraint {
				Axis::Both => self.dragged_distance,
				Axis::X => DVec2::new(self.dragged_distance.x, 0.),
				Axis::Y => DVec2::new(0., self.dragged_distance.y),
			}
		};
		let displacement = transform.inverse().transform_vector2(displacement);
		if increment_mode { displacement.round() } else { displacement }
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

	pub fn negate(self) -> Self {
		let dragged_distance = -self.dragged_distance;
		Self { dragged_distance, ..self }
	}

	pub fn with_constraint(self, target: Axis, local: bool) -> (Self, bool) {
		let (constraint, local) = self.constraint.contrainted_to_axis(target, local);
		(Self { constraint, ..self }, local)
	}
}

#[derive(Default, Debug, Clone, PartialEq, Copy)]
pub struct Rotation {
	pub dragged_angle: f64,
	pub typed_angle: Option<f64>,
}

impl Rotation {
	pub fn to_f64(self, increment_mode: bool) -> f64 {
		if let Some(value) = self.typed_angle {
			value.to_radians()
		} else if increment_mode {
			let increment_resolution = ROTATE_INCREMENT.to_radians();
			(self.dragged_angle / increment_resolution).round() * increment_resolution
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

	pub fn negate(self) -> Self {
		let dragged_angle = -self.dragged_angle;
		Self { dragged_angle, ..self }
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
	pub fn to_f64(self, increment: bool) -> f64 {
		let factor = if let Some(value) = self.typed_factor { value } else { self.dragged_factor };
		if increment { (factor / SCALE_INCREMENT).round() * SCALE_INCREMENT } else { factor }
	}

	pub fn to_dvec(self, increment_mode: bool) -> DVec2 {
		let factor = self.to_f64(increment_mode);

		match self.constraint {
			Axis::Both => DVec2::splat(factor),
			Axis::X => DVec2::new(factor, 1.),
			Axis::Y => DVec2::new(1., factor),
		}
	}

	pub fn negate(self) -> Self {
		let dragged_factor = -self.dragged_factor;
		Self { dragged_factor, ..self }
	}

	#[must_use]
	pub fn increment_amount(self, delta: f64) -> Self {
		Self {
			dragged_factor: (self.dragged_factor + delta),
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

	pub fn with_constraint(self, target: Axis, local: bool) -> (Self, bool) {
		let (constraint, local) = self.constraint.contrainted_to_axis(target, local);
		(Self { constraint, ..self }, local)
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

#[derive(Debug, Clone, PartialEq, Copy, serde::Serialize, serde::Deserialize)]
pub enum TransformType {
	Grab,
	Rotate,
	Scale,
}

impl TransformType {
	pub fn equivalent_to(&self, operation: TransformOperation) -> bool {
		matches!(
			(operation, self),
			(TransformOperation::Scaling(_), TransformType::Scale) | (TransformOperation::Grabbing(_), TransformType::Grab) | (TransformOperation::Rotating(_), TransformType::Rotate)
		)
	}
}

impl TransformOperation {
	#[allow(clippy::too_many_arguments)]
	pub fn apply_transform_operation(&self, selected: &mut Selected, increment_mode: bool, local: bool, quad: Quad, transform: DAffine2, pivot: DVec2, local_transform: DAffine2) {
		let local_axis_transform_angle = (quad.top_left() - quad.top_right()).to_angle();
		if self != &TransformOperation::None {
			let transformation = match self {
				TransformOperation::Grabbing(translation) => {
					let translate = DAffine2::from_translation(transform.transform_vector2(translation.to_dvec(local_transform, increment_mode)));
					if local {
						let resolved_angle = if local_axis_transform_angle > 0. {
							local_axis_transform_angle
						} else {
							local_axis_transform_angle - PI
						};
						DAffine2::from_angle(resolved_angle) * translate * DAffine2::from_angle(-resolved_angle)
					} else {
						translate
					}
				}
				TransformOperation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64(increment_mode)),
				TransformOperation::Scaling(scale) => {
					if local {
						DAffine2::from_angle(local_axis_transform_angle) * DAffine2::from_scale(scale.to_dvec(increment_mode)) * DAffine2::from_angle(-local_axis_transform_angle)
					} else {
						DAffine2::from_scale(scale.to_dvec(increment_mode))
					}
				}
				TransformOperation::None => unreachable!(),
			};

			selected.update_transforms(transformation, Some(pivot), Some(*self));
			self.hints(selected.responses, local);
		}
	}

	pub fn axis_constraint(&self) -> Axis {
		match self {
			TransformOperation::Grabbing(grabbing) => grabbing.constraint,
			TransformOperation::Scaling(scaling) => scaling.constraint,
			_ => Axis::Both,
		}
	}

	pub fn can_begin_typing(&self) -> bool {
		self.is_constraint_to_axis() || !matches!(self, TransformOperation::Grabbing(_))
	}

	#[allow(clippy::too_many_arguments)]
	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected, increment_mode: bool, mut local: bool, quad: Quad, transform: DAffine2, pivot: DVec2, local_transform: DAffine2) -> bool {
		(*self, local) = match self {
			TransformOperation::Grabbing(translation) => {
				let (translation, local) = translation.with_constraint(axis, local);
				(TransformOperation::Grabbing(translation), local)
			}
			TransformOperation::Scaling(scale) => {
				let (scale, local) = scale.with_constraint(axis, local);
				(TransformOperation::Scaling(scale), local)
			}
			_ => (*self, false),
		};
		self.apply_transform_operation(selected, increment_mode, local, quad, transform, pivot, local_transform);
		local
	}

	#[allow(clippy::too_many_arguments)]
	pub fn grs_typed(&mut self, typed: Option<f64>, selected: &mut Selected, increment_mode: bool, local: bool, quad: Quad, transform: DAffine2, pivot: DVec2, local_transform: DAffine2) {
		match self {
			TransformOperation::None => (),
			TransformOperation::Grabbing(translation) => translation.typed_distance = typed,
			TransformOperation::Rotating(rotation) => rotation.typed_angle = typed,
			TransformOperation::Scaling(scale) => scale.typed_factor = typed,
		};

		self.apply_transform_operation(selected, increment_mode, local, quad, transform, pivot, local_transform);
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>, local: bool) {
		use crate::messages::input_mapper::utility_types::input_keyboard::{Key, MouseMotion};
		use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

		let mut input_hints = Vec::new();
		let clear_constraint = "Clear Constraint";
		match self.axis_constraint() {
			Axis::Both => {
				input_hints.push(HintInfo::keys([Key::KeyX], "X-Axis Constraint"));
				input_hints.push(HintInfo::keys([Key::KeyY], "Y-Axis Constraint"));
			}
			Axis::X => {
				let x_label = if local { clear_constraint } else { "Local X-Axis Constraint" };
				input_hints.push(HintInfo::keys([Key::KeyX], x_label));
				input_hints.push(HintInfo::keys([Key::KeyY], "Y-Axis Constraint"));
				if !local {
					input_hints.push(HintInfo::keys([Key::KeyX, Key::KeyX], clear_constraint));
				}
			}
			Axis::Y => {
				let y_label = if local { clear_constraint } else { "Local Y-Axis Constraint" };
				input_hints.push(HintInfo::keys([Key::KeyX], "X-Axis Constraint"));
				input_hints.push(HintInfo::keys([Key::KeyY], y_label));
				if !local {
					input_hints.push(HintInfo::keys([Key::KeyY, Key::KeyY], clear_constraint));
				}
			}
		}

		let grs_hint_group = match self {
			TransformOperation::None => unreachable!(),
			TransformOperation::Scaling(_) => HintGroup(vec![HintInfo::multi_keys([[Key::KeyG], [Key::KeyR]], "Grab/Rotate Selected")]),
			TransformOperation::Grabbing(_) => HintGroup(vec![HintInfo::multi_keys([[Key::KeyR], [Key::KeyS]], "Rotate/Scale Selected")]),
			TransformOperation::Rotating(_) => HintGroup(vec![HintInfo::multi_keys([[Key::KeyG], [Key::KeyS]], "Grab/Scale Selected")]),
		};

		let confirm_and_cancel_group = HintGroup(vec![
			HintInfo::mouse(MouseMotion::Lmb, ""),
			HintInfo::keys([Key::Enter], "Confirm").prepend_slash(),
			HintInfo::mouse(MouseMotion::Rmb, ""),
			HintInfo::keys([Key::Escape], "Cancel").prepend_slash(),
		]);
		let mut hint_groups = vec![confirm_and_cancel_group, grs_hint_group];
		if !self.is_typing() {
			let modifiers = vec![
				HintInfo::keys([Key::Shift], "Slow"),
				HintInfo::keys([Key::Control], if matches!(self, TransformOperation::Rotating(_)) { "15° Increments" } else { "Increments" }),
			];
			hint_groups.push(HintGroup(modifiers));
		}
		if !matches!(self, TransformOperation::Rotating(_)) {
			hint_groups.push(HintGroup(input_hints));
		}
		let mut typing_hints = vec![HintInfo::keys([Key::Minus], "Negate Direction")];
		if self.can_begin_typing() {
			typing_hints.push(HintInfo::keys([Key::NumKeys], "Enter Number"));
			if self.is_typing() {
				typing_hints.push(HintInfo::keys([Key::Backspace], "Delete Digit"));
			}
		}
		hint_groups.push(HintGroup(typing_hints));

		let hint_data = HintData(hint_groups);
		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	pub fn is_constraint_to_axis(&self) -> bool {
		self.axis_constraint() != Axis::Both
	}

	pub fn is_typing(&self) -> bool {
		match self {
			TransformOperation::None => false,
			TransformOperation::Grabbing(translation) => translation.typed_distance.is_some(),
			TransformOperation::Rotating(rotation) => rotation.typed_angle.is_some(),
			TransformOperation::Scaling(scale) => scale.typed_factor.is_some(),
		}
	}

	#[allow(clippy::too_many_arguments)]
	pub fn negate(&mut self, selected: &mut Selected, increment_mode: bool, local: bool, quad: Quad, transform: DAffine2, pivot: DVec2, local_transform: DAffine2) {
		if *self != TransformOperation::None {
			*self = match self {
				TransformOperation::Scaling(scale) => TransformOperation::Scaling(scale.negate()),
				TransformOperation::Rotating(rotation) => TransformOperation::Rotating(rotation.negate()),
				TransformOperation::Grabbing(translation) => TransformOperation::Grabbing(translation.negate()),
				_ => *self,
			};
			self.apply_transform_operation(selected, increment_mode, local, quad, transform, pivot, local_transform);
		}
	}
}

pub struct Selected<'a> {
	pub selected: &'a [LayerNodeIdentifier],
	pub responses: &'a mut VecDeque<Message>,
	pub network_interface: &'a NodeNetworkInterface,
	pub original_transforms: &'a mut OriginalTransforms,
	pub pivot: &'a mut DVec2,
	pub shape_editor: Option<&'a ShapeState>,
	pub tool_type: &'a ToolType,
	// Only for the Pen tool
	pub pen_handle: Option<&'a mut DVec2>,
}

impl<'a> Selected<'a> {
	#[allow(clippy::too_many_arguments)]
	pub fn new(
		original_transforms: &'a mut OriginalTransforms,
		pivot: &'a mut DVec2,
		selected: &'a [LayerNodeIdentifier],
		responses: &'a mut VecDeque<Message>,
		network_interface: &'a NodeNetworkInterface,
		shape_editor: Option<&'a ShapeState>,
		tool_type: &'a ToolType,
		pen_handle: Option<&'a mut DVec2>,
	) -> Self {
		// If user is using the Select tool or Shape tool then use the original layer transforms
		if (*tool_type == ToolType::Select || *tool_type == ToolType::Shape) && (*original_transforms == OriginalTransforms::Path(HashMap::new())) {
			*original_transforms = OriginalTransforms::Layer(HashMap::new());
		}

		original_transforms.update(selected, network_interface, shape_editor);

		Self {
			selected,
			responses,
			network_interface,
			original_transforms,
			pivot,
			shape_editor,
			tool_type,
			pen_handle,
		}
	}

	pub fn center_of_aabb(&mut self) -> DVec2 {
		let [min, max] = self
			.selected
			.iter()
			.filter_map(|&layer| self.network_interface.document_metadata().bounding_box_viewport(layer))
			.reduce(Quad::combine_bounds)
			.unwrap_or_default();
		(min + max) / 2.
	}

	pub fn bounding_box(&mut self) -> Quad {
		let metadata = self.network_interface.document_metadata();

		let mut transform = self
			.network_interface
			.selected_nodes()
			.selected_visible_and_unlocked_layers(self.network_interface)
			.find(|layer| !self.network_interface.is_artboard(&layer.to_node(), &[]))
			.map(|layer| metadata.transform_to_viewport(layer))
			.unwrap_or(DAffine2::IDENTITY);

		if transform.matrix2.determinant().abs() <= f64::EPSILON {
			transform.matrix2 += DMat2::IDENTITY * 1e-4; // TODO: Is this the cleanest way to handle this?
		}

		let bounds = self
			.selected
			.iter()
			.filter_map(|&layer| metadata.bounding_box_with_transform(layer, transform.inverse() * metadata.transform_to_viewport(layer)))
			.reduce(Quad::combine_bounds)
			.unwrap_or_default();

		transform * Quad::from_box(bounds)
	}

	fn transform_layer(document_metadata: &DocumentMetadata, layer: LayerNodeIdentifier, original_transform: Option<&DAffine2>, transformation: DAffine2, responses: &mut VecDeque<Message>) {
		let Some(&original_transform) = original_transform else { return };
		let to = document_metadata.downstream_transform_to_viewport(layer);
		let new = to.inverse() * transformation * to * original_transform;
		responses.add(GraphOperationMessage::TransformSet {
			layer,
			transform: new,
			transform_in: TransformIn::Local,
			skip_rerender: false,
		});
	}

	fn transform_path(
		document_metadata: &DocumentMetadata,
		layer: LayerNodeIdentifier,
		initial_points: &mut InitialPoints,
		transformation: DAffine2,
		responses: &mut VecDeque<Message>,
		transform_operation: Option<TransformOperation>,
	) {
		let viewspace = document_metadata.transform_to_viewport(layer);
		let layerspace_rotation = viewspace.inverse() * transformation;

		for (&point, anchor) in initial_points.anchors.iter_mut() {
			let new_pos_viewport = layerspace_rotation.transform_point2(viewspace.transform_point2(anchor.initial));
			let delta = new_pos_viewport - anchor.current;
			anchor.current += delta;
			let modification_type = VectorModificationType::ApplyPointDelta { point, delta };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		if transform_operation.is_some_and(|transform_operation| matches!(transform_operation, TransformOperation::Scaling(_))) && (initial_points.anchors.len() == 2) {
			return;
		}

		for (&id, handle) in initial_points.handles.iter() {
			let new_pos_viewport = layerspace_rotation.transform_point2(viewspace.transform_point2(handle.initial));
			let relative = initial_points.anchors.get(&handle.anchor).map_or(handle.relative, |anchor| anchor.current);
			let modification_type = id.set_relative_position(new_pos_viewport - relative);
			responses.add(GraphOperationMessage::Vector { layer, modification_type });

			if let Some((id, initial)) = handle.mirror {
				// When the handle is scaled to zero, don't update the mirror handle
				if (new_pos_viewport - relative).length_squared() > f64::EPSILON {
					let direction = viewspace.transform_vector2(new_pos_viewport - relative).try_normalize();
					let length = viewspace.transform_vector2(initial - relative).length();
					let new_relative = direction.map_or(initial - relative, |direction| viewspace.inverse().transform_vector2(-direction * length));
					let modification_type = id.set_relative_position(new_relative);
					responses.add(GraphOperationMessage::Vector { layer, modification_type });
				}
			}
		}
	}

	pub fn apply_transform_pen(&mut self, transformation: DAffine2) {
		if let Some(pen_handle) = &self.pen_handle {
			let final_position = transformation.transform_point2(**pen_handle);
			self.responses.add(PenToolMessage::FinalPosition { final_position });
		}
	}

	pub fn apply_transformation(&mut self, transformation: DAffine2, transform_operation: Option<TransformOperation>) {
		if self.selected.is_empty() {
			return;
		}

		// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
		for layer in self.network_interface.shallowest_unique_layers(&[]) {
			match &mut self.original_transforms {
				OriginalTransforms::Layer(layer_transforms) => Self::transform_layer(self.network_interface.document_metadata(), layer, layer_transforms.get(&layer), transformation, self.responses),
				OriginalTransforms::Path(path_transforms) => {
					if let Some(initial_points) = path_transforms.get_mut(&layer) {
						Self::transform_path(self.network_interface.document_metadata(), layer, initial_points, transformation, self.responses, transform_operation)
					}
				}
			}
		}
	}

	pub fn update_transforms(&mut self, delta: DAffine2, pivot: Option<DVec2>, transform_operation: Option<TransformOperation>) {
		let pivot = DAffine2::from_translation(pivot.unwrap_or(*self.pivot));
		let transformation = pivot * delta * pivot.inverse();
		match self.tool_type {
			ToolType::Pen => self.apply_transform_pen(transformation),
			_ => self.apply_transformation(transformation, transform_operation),
		}
	}

	pub fn revert_operation(&mut self) {
		for layer in self.selected.iter().copied() {
			let original_transform = &self.original_transforms;
			match original_transform {
				OriginalTransforms::Layer(hash) => {
					let Some(matrix) = hash.get(&layer) else { continue };
					self.responses.add(GraphOperationMessage::TransformSet {
						layer,
						transform: *matrix,
						transform_in: TransformIn::Local,
						skip_rerender: false,
					});
				}
				OriginalTransforms::Path(path) => {
					for (&layer, points) in path {
						for (&point, &anchor) in &points.anchors {
							let delta = anchor.initial - anchor.current;
							let modification_type = VectorModificationType::ApplyPointDelta { point, delta };
							self.responses.add(GraphOperationMessage::Vector { layer, modification_type });
						}

						for (&point, &handle) in &points.handles {
							let modification_type = point.set_relative_position(handle.initial - handle.relative);
							self.responses.add(GraphOperationMessage::Vector { layer, modification_type });

							if let Some((id, initial)) = handle.mirror {
								let modification_type = id.set_relative_position(initial - handle.relative);
								self.responses.add(GraphOperationMessage::Vector { layer, modification_type });
							}
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
	pub string: String,
	pub contains_decimal: bool,
	pub negative: bool,
}

const DECIMAL_POINT: u8 = 10;

impl Typing {
	pub fn type_number(&mut self, number: u8) -> Option<f64> {
		self.digits.push(number);
		self.string.push((b'0' + number) as char);

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
		self.string.pop();
		self.evaluate()
	}

	pub fn type_decimal_point(&mut self) -> Option<f64> {
		if !self.contains_decimal {
			self.contains_decimal = true;
			self.digits.push(DECIMAL_POINT);
			self.string.push('.');
		}

		self.evaluate()
	}

	pub fn type_negate(&mut self) -> Option<f64> {
		self.negative = !self.negative;
		if self.negative {
			self.string.insert(0, '-');
		} else {
			self.string.remove(0);
		}

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
		self.string.clear();
		self.contains_decimal = false;
		self.negative = false;
	}
}
