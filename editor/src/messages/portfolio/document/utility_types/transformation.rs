use super::network_interface::NodeNetworkInterface;
use crate::consts::{ROTATE_SNAP_ANGLE, SCALE_SNAP_INTERVAL};
use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
use crate::messages::portfolio::document::utility_types::document_metadata::{DocumentMetadata, LayerNodeIdentifier};
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::graph_modification_utils;
use crate::messages::tool::common_functionality::shape_editor::ShapeState;
use crate::messages::tool::utility_types::ToolType;

use graphene_core::renderer::Quad;
use graphene_core::vector::ManipulatorPointId;
use graphene_core::vector::VectorModificationType;
use graphene_std::vector::{HandleId, PointId};

use glam::{DAffine2, DVec2};
use std::collections::{HashMap, VecDeque};

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

	pub fn update<'a>(&mut self, selected: &'a [LayerNodeIdentifier], network_interface: &NodeNetworkInterface, shape_editor: Option<&'a ShapeState>) {
		let document_metadata = network_interface.document_metadata();

		match self {
			OriginalTransforms::Layer(layer_map) => {
				layer_map.retain(|layer, _| selected.contains(layer));
				for &layer in selected {
					if layer == LayerNodeIdentifier::ROOT_PARENT {
						continue;
					}
					layer_map.entry(layer).or_insert_with(|| document_metadata.upstream_transform(layer.to_node()));
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
					let Some(vector_data) = network_interface.compute_modified_vector(layer) else {
						continue;
					};
					let Some(selected_points) = shape_editor.selected_points_in_layer(layer) else {
						continue;
					};

					// Anchors also move their handles
					let anchor_ids = selected_points.iter().filter_map(|point| point.as_anchor());
					let anchors = anchor_ids.filter_map(|id| vector_data.point_domain.position_from_id(id).map(|pos| (id, AnchorPoint { initial: pos, current: pos })));
					let anchors = anchors.collect();

					let selected_handles = selected_points.iter().filter_map(|point| point.as_handle());
					let anchor_ids = selected_points.iter().filter_map(|point| point.as_anchor());
					let connected_handles = anchor_ids.flat_map(|point| vector_data.all_connected(point));
					let all_handles = selected_handles.chain(connected_handles);

					let handles = all_handles
						.filter_map(|id| {
							let anchor = id.to_manipulator_point().get_anchor(&vector_data)?;
							let initial = id.to_manipulator_point().get_position(&vector_data)?;
							let relative = vector_data.point_domain.position_from_id(anchor)?;
							let other_handle = vector_data
								.other_colinear_handle(id)
								.filter(|other| !selected_points.contains(&other.to_manipulator_point()) && !selected_points.contains(&ManipulatorPointId::Anchor(anchor)));
							let mirror = other_handle.and_then(|id| Some((id, id.to_manipulator_point().get_position(&vector_data)?)));

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
		if self == target {
			if local {
				(Axis::Both, false)
			} else {
				(self, true)
			}
		} else {
			(target, false)
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
	pub fn to_f64(self, snap: bool) -> f64 {
		let factor = if let Some(value) = self.typed_factor { value } else { self.dragged_factor };
		if snap {
			(factor / SCALE_SNAP_INTERVAL).round() * SCALE_SNAP_INTERVAL
		} else {
			factor
		}
	}

	pub fn to_dvec(self, snap: bool) -> DVec2 {
		let factor = self.to_f64(snap);

		match self.constraint {
			Axis::Both => DVec2::splat(factor),
			Axis::X => DVec2::new(factor, 1.),
			Axis::Y => DVec2::new(1., factor),
		}
	}

	pub fn negate(self) -> Self {
		Self {
			dragged_factor: -self.dragged_factor,
			..self
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

impl TransformOperation {
	pub fn apply_transform_operation(&self, selected: &mut Selected, snapping: bool, local: bool, quad: Quad) {
		let quad = quad.0;
		let edge = quad[1] - quad[0];
		if self != &TransformOperation::None {
			let transformation = match self {
				TransformOperation::Grabbing(translation) => {
					if local {
						debug!("{:?}", quad);
						DAffine2::from_angle(edge.to_angle()) * DAffine2::from_translation(translation.to_dvec()) * DAffine2::from_angle(-edge.to_angle())
					} else {
						DAffine2::from_translation(translation.to_dvec())
					}
				}
				TransformOperation::Rotating(rotation) => DAffine2::from_angle(rotation.to_f64(snapping)),
				TransformOperation::Scaling(scale) => {
					if local {
						DAffine2::from_angle(edge.to_angle()) * DAffine2::from_scale(scale.to_dvec(snapping)) * DAffine2::from_angle(-edge.to_angle())
					} else {
						DAffine2::from_scale(scale.to_dvec(snapping))
					}
				}
				TransformOperation::None => unreachable!(),
			};

			selected.update_transforms(transformation);
			self.hints(selected.responses);
		}
	}

	pub fn constrain_axis(&mut self, axis: Axis, selected: &mut Selected, snapping: bool, mut local: bool, quad: Quad) -> bool {
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
		self.apply_transform_operation(selected, snapping, local, quad);
		local
	}

	pub fn grs_typed(&mut self, typed: Option<f64>, selected: &mut Selected, snapping: bool, local: bool, quad: Quad) {
		match self {
			TransformOperation::None => (),
			TransformOperation::Grabbing(translation) => translation.typed_distance = typed,
			TransformOperation::Rotating(rotation) => rotation.typed_angle = typed,
			TransformOperation::Scaling(scale) => scale.typed_factor = typed,
		};

		self.apply_transform_operation(selected, snapping, local, quad);
	}

	pub fn hints(&self, responses: &mut VecDeque<Message>) {
		use crate::messages::input_mapper::utility_types::input_keyboard::Key;
		use crate::messages::tool::utility_types::{HintData, HintGroup, HintInfo};

		let mut input_hints = Vec::new();
		input_hints.push(HintInfo::keys([Key::Shift], "Slow Mode"));
		if matches!(self, TransformOperation::Rotating(_) | TransformOperation::Scaling(_)) {
			input_hints.push(HintInfo::keys([Key::Control], "Snap"));
		}
		if matches!(self, TransformOperation::Grabbing(_) | TransformOperation::Scaling(_)) {
			input_hints.push(HintInfo::keys([Key::KeyX], "Along X Axis"));
			input_hints.push(HintInfo::keys([Key::KeyY], "Along Y Axis"));
		}

		let hint_data = HintData(vec![HintGroup(input_hints)]);
		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	pub fn negate(&mut self, selected: &mut Selected, snapping: bool, local: bool, quad: Quad) {
		if *self != TransformOperation::None {
			*self = match self {
				TransformOperation::Scaling(scale) => TransformOperation::Scaling(scale.negate()),
				_ => *self,
			};
			self.apply_transform_operation(selected, snapping, local, quad);
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
	) -> Self {
		// If user is using the Select tool then use the original layer transforms
		if (*tool_type == ToolType::Select) && (*original_transforms == OriginalTransforms::Path(HashMap::new())) {
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
		}
	}

	pub fn mean_average_of_pivots(&mut self) -> DVec2 {
		let xy_summation = self
			.selected
			.iter()
			.map(|&layer| graph_modification_utils::get_viewport_pivot(layer, self.network_interface))
			.reduce(|a, b| a + b)
			.unwrap_or_default();

		xy_summation / self.selected.len() as f64
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
		self.selected
			.iter()
			.filter_map(|&layer| {
				self.network_interface
					.document_metadata()
					.bounding_box_with_transform(layer, DAffine2::IDENTITY)
					.map(|bounds| self.network_interface.document_metadata().transform_to_document(layer) * Quad::from_box(bounds))
			})
			.last()
			.unwrap_or_default()
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

	fn transform_path(document_metadata: &DocumentMetadata, layer: LayerNodeIdentifier, initial_points: &mut InitialPoints, transformation: DAffine2, responses: &mut VecDeque<Message>) {
		let viewspace = document_metadata.transform_to_viewport(layer);
		let layerspace_rotation = viewspace.inverse() * transformation;

		for (&point, anchor) in initial_points.anchors.iter_mut() {
			let new_pos_viewport = layerspace_rotation.transform_point2(viewspace.transform_point2(anchor.initial));
			let delta = new_pos_viewport - anchor.current;
			anchor.current += delta;
			let modification_type = VectorModificationType::ApplyPointDelta { point, delta };
			responses.add(GraphOperationMessage::Vector { layer, modification_type });
		}

		for (&id, handle) in initial_points.handles.iter() {
			let new_pos_viewport = layerspace_rotation.transform_point2(viewspace.transform_point2(handle.initial));
			let relative = initial_points.anchors.get(&handle.anchor).map_or(handle.relative, |anchor| anchor.current);
			let modification_type = id.set_relative_position(new_pos_viewport - relative);
			responses.add(GraphOperationMessage::Vector { layer, modification_type });

			if let Some((id, initial)) = handle.mirror {
				let direction = viewspace.transform_vector2(new_pos_viewport - relative).try_normalize();
				let length = viewspace.transform_vector2(initial - relative).length();
				let new_relative = direction.map_or(initial - relative, |direction| viewspace.inverse().transform_vector2(-direction * length));
				let modification_type = id.set_relative_position(new_relative);
				responses.add(GraphOperationMessage::Vector { layer, modification_type });
			}
		}
	}

	pub fn apply_transformation(&mut self, transformation: DAffine2) {
		if !self.selected.is_empty() {
			// TODO: Cache the result of `shallowest_unique_layers` to avoid this heavy computation every frame of movement, see https://github.com/GraphiteEditor/Graphite/pull/481
			for layer in self.network_interface.shallowest_unique_layers(&[]) {
				match &mut self.original_transforms {
					OriginalTransforms::Layer(layer_transforms) => {
						Self::transform_layer(self.network_interface.document_metadata(), layer, layer_transforms.get(&layer), transformation, self.responses)
					}
					OriginalTransforms::Path(path_transforms) => {
						if let Some(initial_points) = path_transforms.get_mut(&layer) {
							Self::transform_path(self.network_interface.document_metadata(), layer, initial_points, transformation, self.responses)
						}
					}
				}
			}
		}
	}

	pub fn update_transforms(&mut self, delta: DAffine2) {
		let pivot = DAffine2::from_translation(*self.pivot);
		let transformation = pivot * delta * pivot.inverse();
		self.apply_transformation(transformation);
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
