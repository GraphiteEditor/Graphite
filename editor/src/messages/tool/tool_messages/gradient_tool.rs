use super::tool_prelude::*;
use crate::consts::{
	COLOR_OVERLAY_BLUE, DRAG_THRESHOLD, GRADIENT_MIDPOINT_DIAMOND_RADIUS, GRADIENT_MIDPOINT_MAX, GRADIENT_MIDPOINT_MIN, GRADIENT_STOP_MIN_VIEWPORT_GAP, LINE_ROTATE_SNAP_ANGLE,
	MANIPULATOR_GROUP_MARKER_SIZE, SEGMENT_INSERTION_DISTANCE, SEGMENT_OVERLAY_SIZE, SELECTION_THRESHOLD,
};
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::overlays::utility_types::{GizmoEmphasis, OverlayContext};
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::{FlowType, NodeNetworkInterface};
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::graph_modification_utils::{
	self, NodeGraphLayer, get_fill_node_id_with_direct_fill_input, get_gradient_stops, get_upstream_gradient_value_node_id, gradient_chain_target_input,
};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};
use glam::DMat2;
use graph_craft::document::value::TaggedValue;
use graphene_std::color::SRGBA8;
use graphene_std::raster::color::Color;
use graphene_std::vector::style::{
	FillChoice, FillChoiceUI, GradientSpreadMethod, GradientStop, GradientStops, GradientStopsUI, GradientType, build_transform_with_y_preservation,
};

#[derive(Default, ExtractField)]
pub struct GradientTool {
	fsm_state: GradientToolFsmState,
	data: GradientToolData,
	options: GradientOptions,
}

#[derive(Default)]
pub struct GradientOptions {
	gradient_type: GradientType,
	spread_method: GradientSpreadMethod,
}

#[impl_message(Message, ToolMessage, Gradient)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum GradientToolMessage {
	// Standard messages
	Abort,
	Overlays { context: OverlayContext },
	SelectionChanged,
	WorkingColorChanged,

	// Tool-specific messages
	DeleteStop,
	DoubleClick,
	InsertStop,
	PointerDown,
	PointerMove { constrain_axis: Key, lock_angle: Key },
	PointerOutsideViewport { constrain_axis: Key, lock_angle: Key },
	PointerUp,
	StartTransactionForColorStop,
	CommitTransactionForColorStop,
	CloseStopColorPicker,
	UpdateStopColor { color: Color },
	UpdateStops { stops: GradientStopsUI },
	UpdateOptions { options: GradientOptionsUpdate },
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum GradientOptionsUpdate {
	Type(GradientType),
	ReverseStops,
	ReverseDirection,
	SetSpreadMethod(GradientSpreadMethod),
}

impl ToolMetadata for GradientTool {
	fn icon_name(&self) -> String {
		"GeneralGradientTool".into()
	}
	fn tooltip_label(&self) -> String {
		"Gradient Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Gradient
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for GradientTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		match message {
			ToolMessage::Gradient(GradientToolMessage::UpdateOptions { options }) => match options {
				GradientOptionsUpdate::Type(gradient_type) => {
					self.options.gradient_type = gradient_type;
					apply_gradient_update(
						&mut self.data,
						context,
						responses,
						|(_gradient, appearance)| appearance.gradient_type != gradient_type,
						|(_gradient, appearance)| appearance.gradient_type = gradient_type,
					);
					responses.add(ToolMessage::UpdateHints);
					responses.add(ToolMessage::UpdateCursor);
				}
				GradientOptionsUpdate::ReverseStops => {
					apply_gradient_update(&mut self.data, context, responses, |_| true, |(gradient, _appearance)| *gradient = gradient.reversed());
				}
				GradientOptionsUpdate::ReverseDirection => apply_gradient_update(
					&mut self.data,
					context,
					responses,
					|_| true,
					|(_gradient, appearance)| {
						let reverse = DAffine2 {
							matrix2: -DMat2::IDENTITY,
							translation: DVec2::X,
						};
						appearance.transform *= reverse;
					},
				),
				GradientOptionsUpdate::SetSpreadMethod(spread_method) => {
					self.options.spread_method = spread_method;
					apply_gradient_update(
						&mut self.data,
						context,
						responses,
						|(_gradient, appearance)| appearance.spread_method != spread_method,
						|(_gradient, appearance)| appearance.spread_method = spread_method,
					);
				}
			},
			ToolMessage::Gradient(GradientToolMessage::StartTransactionForColorStop) => {
				if self.data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
				}
				responses.add(DocumentMessage::StartTransaction);
				self.data.color_picker_transaction_open = true;
			}
			ToolMessage::Gradient(GradientToolMessage::CommitTransactionForColorStop) => {
				if self.data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
					self.data.color_picker_transaction_open = false;
				}
			}
			ToolMessage::Gradient(GradientToolMessage::UpdateStopColor { color }) => {
				if let Some(stop_index) = self.data.color_picker_editing_color_stop
					&& let Some(selected_gradient) = &mut self.data.selected_gradient
					&& stop_index < selected_gradient.gradient.color.len()
				{
					selected_gradient.gradient.color[stop_index] = color;
					selected_gradient.render_gradient(responses);
					responses.add(PropertiesPanelMessage::Refresh);
				}
			}
			ToolMessage::Gradient(GradientToolMessage::UpdateStops { stops }) => {
				apply_stops_update(&mut self.data, context, responses, GradientStops::from(&stops));
			}
			ToolMessage::Gradient(GradientToolMessage::CloseStopColorPicker) => {
				if self.data.color_picker_transaction_open {
					responses.add(DocumentMessage::EndTransaction);
					self.data.color_picker_transaction_open = false;
				}
				self.data.color_picker_editing_color_stop = None;
			}
			ToolMessage::Gradient(GradientToolMessage::WorkingColorChanged) => {
				let primary = context.global_tool_data.primary_color;
				let secondary = context.global_tool_data.secondary_color;

				if self.data.primary_color != primary || self.data.secondary_color != secondary {
					self.data.primary_color = primary;
					self.data.secondary_color = secondary;

					if !self.data.has_selected_gradient {
						responses.add(ToolMessage::RefreshToolOptions);
					}
				}
			}
			_ => {
				self.fsm_state.process_event(message, &mut self.data, context, &self.options, responses, false);

				// Reading from the layer (not from the in-progress drag state) keeps the control bar widgets current across selection changes, not just drags
				let (current_layer, current_gradient) = current_layer_and_gradient(context.document);

				let mut needs_refresh = false;
				if let Some((_gradient, appearance)) = &current_gradient {
					if self.options.gradient_type != appearance.gradient_type {
						self.options.gradient_type = appearance.gradient_type;
						needs_refresh = true;
					}
					if self.options.spread_method != appearance.spread_method {
						self.options.spread_method = appearance.spread_method;
						needs_refresh = true;
					}
				}

				let has_gradient = current_gradient.is_some();
				if has_gradient != self.data.has_selected_gradient {
					self.data.has_selected_gradient = has_gradient;
					needs_refresh = true;
				}

				let new_stops = current_gradient.as_ref().map(|(gradient, _appearance)| gradient.clone());
				if self.data.current_gradient_stops != new_stops {
					self.data.current_gradient_stops = new_stops;
					needs_refresh = true;
				}

				let new_orientation = match (current_layer, &current_gradient) {
					(Some(layer), Some((_gradient, appearance))) => {
						let transform = gradient_space_transform(layer, context.document) * appearance.transform;
						!graph_modification_utils::gradient_orientation_rightward(transform)
					}
					_ => true,
				};
				if new_orientation != self.data.gradient_orientation_rightward {
					self.data.gradient_orientation_rightward = new_orientation;
					needs_refresh = true;
				}

				if needs_refresh {
					responses.add(ToolMessage::RefreshToolOptions);
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut common = actions!(GradientToolMessageDiscriminant;
			PointerDown,
			PointerUp,
			PointerMove,
			DoubleClick,
			Abort,
		);

		// Only intercept Delete/Backspace (`DeleteStop`) while a deletable stop or midpoint is selected
		if self.data.selected_gradient.as_ref().is_some_and(|selected| !matches!(selected.dragging, GradientDragTarget::New)) {
			common.extend(actions!(GradientToolMessageDiscriminant; DeleteStop));
		}

		common
	}
}

impl LayoutHolder for GradientTool {
	fn layout(&self) -> Layout {
		let mut widgets: Vec<WidgetInstance> = Vec::new();

		let gradient_type = RadioInput::new(vec![
			RadioEntryData::new("Linear").label("Linear").tooltip_label("Linear Gradient").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::Type(GradientType::Linear),
				}
				.into()
			}),
			RadioEntryData::new("Radial").label("Radial").tooltip_label("Radial Gradient").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::Type(GradientType::Radial),
				}
				.into()
			}),
		])
		.selected_index(Some((self.options.gradient_type == GradientType::Radial) as u32))
		.widget_instance();

		// Display priority: the selected layer's stops, then any user-customized tool default, then the working colors
		let stops_value = self
			.data
			.current_gradient_stops
			.clone()
			.or_else(|| self.data.default_gradient_stops.clone())
			.map(FillChoice::Gradient)
			.unwrap_or_else(|| {
				FillChoice::Gradient(GradientStops::new([
					GradientStop {
						position: 0.,
						midpoint: 0.5,
						color: self.data.primary_color,
					},
					GradientStop {
						position: 1.,
						midpoint: 0.5,
						color: self.data.secondary_color,
					},
				]))
			});
		let stops_widget = ColorInput::new(FillChoiceUI::from(&stops_value))
			.allow_none(false)
			.narrow(true)
			.tooltip_label("Gradient Stops")
			.tooltip_description("Edit the gradient's color stops.")
			.on_update(|input: &ColorInput| {
				let stops = input.value.as_gradient().cloned().unwrap_or_default();
				GradientToolMessage::UpdateStops { stops }.into()
			})
			.on_commit(|_| DocumentMessage::AddTransaction.into())
			.widget_instance();

		let reverse_stops = IconButton::new("Reverse", 24)
			.tooltip_label("Reverse Stops")
			.tooltip_description("Reverse the gradient color stops.")
			.disabled(!self.data.has_selected_gradient)
			.on_update(|_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::ReverseStops,
				}
				.into()
			})
			.widget_instance();

		let spread_method = RadioInput::new(vec![
			RadioEntryData::new("Pad").label("Pad").tooltip_label("Pad Spread Method").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Pad),
				}
				.into()
			}),
			RadioEntryData::new("Reflect").label("Reflect").tooltip_label("Reflect Spread Method").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Reflect),
				}
				.into()
			}),
			RadioEntryData::new("Repeat").label("Repeat").tooltip_label("Repeat Spread Method").on_update(move |_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Repeat),
				}
				.into()
			}),
		])
		.selected_index(Some(self.options.spread_method as u32))
		.widget_instance();

		let reverse_direction_icon = if self.data.gradient_orientation_rightward {
			"ReverseRadialGradientToRight"
		} else {
			"ReverseRadialGradientToLeft"
		};
		let reverse_direction = IconButton::new(reverse_direction_icon, 24)
			.tooltip_label("Reverse Direction")
			.tooltip_description("Reverse which end the gradient radiates from.")
			.disabled(!self.data.has_selected_gradient)
			.on_update(|_| {
				GradientToolMessage::UpdateOptions {
					options: GradientOptionsUpdate::ReverseDirection,
				}
				.into()
			})
			.widget_instance();

		widgets.extend([
			stops_widget,
			Separator::new(SeparatorStyle::Related).widget_instance(),
			reverse_stops,
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			gradient_type,
			Separator::new(SeparatorStyle::Unrelated).widget_instance(),
			spread_method,
			Separator::new(SeparatorStyle::Related).widget_instance(),
			reverse_direction,
		]);

		Layout(vec![LayoutGroup::row(widgets)])
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GradientToolFsmState {
	Ready { hovering: GradientHoverTarget, selected: GradientSelectedTarget },
	Drawing { drag_hint: GradientDragHintState },
}

impl Default for GradientToolFsmState {
	fn default() -> Self {
		Self::Ready {
			hovering: GradientHoverTarget::None,
			selected: GradientSelectedTarget::None,
		}
	}
}

/// Computes the transform from gradient space to viewport space.
fn gradient_space_transform(layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> DAffine2 {
	graph_modification_utils::gradient_space_transform(layer, &document.network_interface)
}

/// Viewport positions of the gradient's start (unit param 0) and end (unit param 1) handles.
fn gradient_handle_positions(unit_to_viewport: DAffine2) -> (DVec2, DVec2) {
	(unit_to_viewport.transform_point2(DVec2::ZERO), unit_to_viewport.transform_point2(DVec2::X))
}

#[derive(Debug, PartialEq)]
enum GradientSource {
	Direct,
	Chain,
}

/// Get the gradient with appearance information from Fill node values, or the chain connected to Fill node / layer.
fn resolve_gradient(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<(GradientStops, GradientAppearance, GradientSource)> {
	if let Some(stops) = get_gradient_stops(layer, network_interface) {
		// A Fill node holding a direct gradient value decodes through the shared reader
		if let Some(fill_id) = get_fill_node_id_with_direct_fill_input(layer, network_interface) {
			let fill_node = network_interface.document_network().nodes.get(&fill_id)?;
			let gradient = graph_modification_utils::read_fill_node_gradient(fill_node, || network_interface.document_metadata().nonzero_bounding_box(layer))?;

			return Some((
				gradient.stops,
				GradientAppearance {
					gradient_type: gradient.gradient_type,
					spread_method: gradient.spread_method,
					transform: gradient.transform,
				},
				GradientSource::Direct,
			));
		}

		// Then, try to construct a gradient out of a chain, which is directly connected to a Fill node or a layer
		let appearance = read_gradient_chain_state(layer, network_interface);
		Some((stops, appearance, GradientSource::Chain))
	} else {
		None
	}
}

// FIXME: consider rename this and merge this to GradientOptions if possible
#[derive(Clone, Copy, Debug, Default)]
struct GradientAppearance {
	transform: DAffine2,
	gradient_type: GradientType,
	spread_method: GradientSpreadMethod,
}

/// Resolve the gradient transform, type, and spread method by walking the chain feeding the layer. Transform composes all
/// 'Transform' nodes. Type and spread method come from the closest-to-layer node of each kind, or the type default.
fn read_gradient_chain_state(layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> GradientAppearance {
	let target_input = gradient_chain_target_input(layer, network_interface);
	let walk_from = network_interface.upstream_output_connector(&target_input, &[]).and_then(|out| out.node_id()).unwrap_or(layer.to_node());

	let transform_reference = DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER);
	let gradient_type_reference = DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_type::IDENTIFIER);
	let spread_method_reference = DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::spread_method::IDENTIFIER);

	let mut transforms_downstream_to_upstream: Vec<DAffine2> = Vec::new();
	let mut gradient_type: Option<GradientType> = None;
	let mut spread_method: Option<GradientSpreadMethod> = None;

	for node_id in network_interface
		.upstream_flow_back_from_nodes(vec![walk_from], &[], FlowType::HorizontalFlow)
		.skip_while(|node_id| network_interface.is_layer(node_id, &[]))
		.take_while(|node_id| !network_interface.is_layer(node_id, &[]))
	{
		let Some(reference) = network_interface.reference(&node_id, &[]) else { continue };
		let Some(document_node) = network_interface.document_network().nodes.get(&node_id) else {
			continue;
		};

		if reference == transform_reference {
			transforms_downstream_to_upstream.push(read_transform_node_value(&document_node.inputs));
		} else if reference == gradient_type_reference
			&& gradient_type.is_none()
			&& let Some(TaggedValue::GradientType(value)) = document_node.inputs.get(1).and_then(|input| input.as_value())
		{
			gradient_type = Some(*value);
		} else if reference == spread_method_reference
			&& spread_method.is_none()
			&& let Some(TaggedValue::GradientSpreadMethod(value)) = document_node.inputs.get(1).and_then(|input| input.as_value())
		{
			spread_method = Some(*value);
		}
	}

	// Iteration order [T_n, ..., T_1] is the matrix-product order, so the fold yields T_n * ... * T_1
	let composed_transform = transforms_downstream_to_upstream.into_iter().fold(DAffine2::IDENTITY, |acc, matrix| acc * matrix);

	GradientAppearance {
		transform: composed_transform,
		gradient_type: gradient_type.unwrap_or_default(),
		spread_method: spread_method.unwrap_or_default(),
	}
}

/// Reconstruct the `DAffine2` produced by a 'Transform' node from its translation, rotation, scale, and skew inputs.
fn read_transform_node_value(inputs: &[graph_craft::document::NodeInput]) -> DAffine2 {
	let translation = inputs
		.get(1)
		.and_then(|input| input.as_value())
		.and_then(|value| if let TaggedValue::DVec2(v) = value { Some(*v) } else { None })
		.unwrap_or(DVec2::ZERO);
	let rotation_degrees = inputs
		.get(2)
		.and_then(|input| input.as_value())
		.and_then(|value| if let TaggedValue::F64(v) = value { Some(*v) } else { None })
		.unwrap_or(0.);
	let scale = inputs
		.get(3)
		.and_then(|input| input.as_value())
		.and_then(|value| if let TaggedValue::DVec2(v) = value { Some(*v) } else { None })
		.unwrap_or(DVec2::ONE);
	let skew = inputs
		.get(4)
		.and_then(|input| input.as_value())
		.and_then(|value| if let TaggedValue::DVec2(v) = value { Some(*v) } else { None })
		.unwrap_or(DVec2::ZERO);

	let trs = DAffine2::from_scale_angle_translation(scale, rotation_degrees.to_radians(), translation);
	let skew_matrix = DAffine2::from_cols_array(&[1., skew.y.to_radians().tan(), skew.x.to_radians().tan(), 1., 0., 0.]);
	trs * skew_matrix
}

/// Whether two adjacent stops are too closely packed in viewport space for a midpoint diamond to be shown or interacted with.
fn midpoint_hidden_by_proximity(left_stop_pos: f64, right_stop_pos: f64, viewport_line_length: f64) -> bool {
	(right_stop_pos - left_stop_pos) * viewport_line_length < GRADIENT_STOP_MIN_VIEWPORT_GAP * 2.
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum GradientDragTarget {
	Start,
	#[default]
	End,
	Stop(usize),
	Midpoint(usize),
	New,
}

/// Contains information about the selected gradient handle
#[derive(Clone, Debug, Default)]
struct SelectedGradient {
	layer: Option<LayerNodeIdentifier>,
	dragging: GradientDragTarget,
	/// Transform from the geometry's local gradient space to viewport space.
	gradient_space_transform: DAffine2,
	gradient: GradientStops,
	appearance: GradientAppearance,
	initial_gradient: GradientStops,
	/// Transform from unit [0, 1] line to the geometry's local gradient space, the snapshot from `GradientAppearance.transform`.
	initial_gradient_transform: DAffine2,
	is_gradient_chain: bool,
}

fn calculate_insertion(start: DVec2, end: DVec2, stops: &GradientStops, mouse: DVec2) -> Option<f64> {
	let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();
	let projection = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

	if distance.abs() < SEGMENT_INSERTION_DISTANCE && (0. ..=1.).contains(&projection) {
		for stop in stops {
			let stop_pos = start.lerp(end, stop.position);
			if stop_pos.distance_squared(mouse) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2) {
				return None;
			}
		}
		if start.distance_squared(mouse) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2) || end.distance_squared(mouse) < (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2) {
			return None;
		}

		// Don't insert when clicking near a (currently visible) midpoint diamond
		let line_length = start.distance(end);
		for i in 0..stops.position.len().saturating_sub(1) {
			let left = stops.position[i];
			let right = stops.position[i + 1];

			if midpoint_hidden_by_proximity(left, right, line_length) {
				continue;
			}

			let midpoint_pos = left + stops.midpoint[i] * (right - left);
			let midpoint_viewport = start.lerp(end, midpoint_pos);
			if midpoint_viewport.distance_squared(mouse) < GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2) {
				return None;
			}
		}

		return Some(projection);
	}

	None
}

impl SelectedGradient {
	pub fn new(gradient: GradientStops, appearance: GradientAppearance, source: GradientSource, layer: LayerNodeIdentifier, document: &DocumentMessageHandler) -> Self {
		let gradient_space_transform = gradient_space_transform(layer, document);
		Self {
			layer: Some(layer),
			gradient_space_transform,
			gradient: gradient.clone(),
			appearance,
			dragging: GradientDragTarget::End,
			initial_gradient: gradient,
			initial_gradient_transform: appearance.transform,
			is_gradient_chain: source == GradientSource::Chain,
		}
	}

	#[allow(clippy::too_many_arguments)]
	pub fn update_gradient(
		&mut self,
		mut mouse: DVec2,
		responses: &mut VecDeque<Message>,
		snap_rotate: bool,
		lock_angle: bool,
		gradient_type: GradientType,
		drag_start: DVec2,
		snap_data: SnapData,
		snap_manager: &mut SnapManager,
		gradient_angle: &mut f64,
	) {
		if mouse.distance(drag_start) < DRAG_THRESHOLD {
			self.gradient = self.initial_gradient.clone();
			self.appearance.transform = self.initial_gradient_transform;
			self.render_gradient(responses);
			return;
		}

		self.appearance.gradient_type = gradient_type;

		let anchor_point = || {
			let (start, end) = self.viewport_handle_positions();
			if self.dragging == GradientDragTarget::Start {
				end
			} else if self.dragging == GradientDragTarget::New {
				drag_start
			} else {
				start
			}
		};

		if (lock_angle || snap_rotate) && matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start | GradientDragTarget::New) {
			let point = anchor_point();
			let delta = point - mouse;

			let mut angle = -delta.angle_to(DVec2::X);

			if lock_angle {
				angle = *gradient_angle;
			} else if snap_rotate {
				let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
				angle = (angle / snap_resolution).round() * snap_resolution;
			}

			*gradient_angle = angle;

			if lock_angle {
				let unit_direction = DVec2::new(angle.cos(), angle.sin());
				let length = delta.dot(unit_direction);
				mouse = point - length * unit_direction;
			} else {
				let length = delta.length();
				let rotated = DVec2::new(length * angle.cos(), length * angle.sin());
				mouse = point - rotated;
			}
		} else {
			// Update stored angle even when not constraining (for dragging endpoints and drawing a new gradient)
			if matches!(self.dragging, GradientDragTarget::End | GradientDragTarget::Start | GradientDragTarget::New) {
				let point = anchor_point();
				let delta = point - mouse;
				*gradient_angle = -delta.angle_to(DVec2::X);
			}

			// Basic point snapping when not angle-constraining
			let document_to_viewport = snap_data.document.metadata().document_to_viewport;
			let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
			let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);
			let snapped = snap_manager.free_snap(&snap_data, &point_candidate, SnapTypeConfiguration::default());
			if snapped.is_snapped() {
				mouse = document_to_viewport.transform_point2(snapped.snapped_point_document);
			}
			snap_manager.update_indicator(snapped);
		}

		let local_mouse = self.gradient_space_transform.inverse().transform_point2(mouse);
		let local_start = self.appearance.transform.transform_point2(DVec2::ZERO);
		let local_end = self.appearance.transform.transform_point2(DVec2::X);

		let old_transform = self.appearance.transform;
		let create_new_gradient_transform = |new_start: DVec2, new_end: DVec2| build_transform_with_y_preservation(old_transform, new_start, new_end);

		match self.dragging {
			GradientDragTarget::Start => {
				self.appearance.transform = create_new_gradient_transform(local_mouse, local_end);
			}
			GradientDragTarget::End => {
				self.appearance.transform = create_new_gradient_transform(local_start, local_mouse);
			}
			GradientDragTarget::New => {
				self.appearance.transform = create_new_gradient_transform(self.gradient_space_transform.inverse().transform_point2(drag_start), local_mouse);
			}
			GradientDragTarget::Stop(s) => {
				let document_to_viewport = snap_data.document.metadata().document_to_viewport;

				let (viewport_start, viewport_end) = self.viewport_handle_positions();

				let line_length = viewport_start.distance(viewport_end);
				if line_length < f64::EPSILON {
					self.render_gradient(responses);
					return;
				}

				let (document_start, document_end) = (
					document_to_viewport.inverse().transform_point2(viewport_start),
					document_to_viewport.inverse().transform_point2(viewport_end),
				);

				let constraint = SnapConstraint::Line {
					origin: document_start,
					direction: document_end - document_start,
				};

				let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
				let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);

				let snapped = snap_manager.constrained_snap(&snap_data, &point_candidate, constraint, SnapTypeConfiguration::default());

				let projected_mouse_document = if snapped.is_snapped() {
					snapped.snapped_point_document
				} else {
					constraint.projection(document_mouse)
				};
				let projected_mouse = document_to_viewport.transform_point2(projected_mouse_document);
				snap_manager.update_indicator(snapped);

				// Calculate the new position by finding the closest point on the line
				let new_pos = ((viewport_end - viewport_start).angle_to(projected_mouse - viewport_start)).cos() * viewport_start.distance(projected_mouse) / line_length;

				if !new_pos.is_finite() {
					self.render_gradient(responses);
					return;
				}

				// Allow dragging through other stops (they'll reorder via sort), but clamp near
				// the endpoints at 0 and 1 if a different color stop already occupies that position
				let min_gap = GRADIENT_STOP_MIN_VIEWPORT_GAP / line_length;
				let last_index = self.gradient.len() - 1;

				let has_other_stop_at_zero = s != 0 && self.gradient.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.);
				let has_other_stop_at_one = s != last_index && self.gradient.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.);

				let left_bound = if has_other_stop_at_zero { min_gap } else { 0. };
				let right_bound = if has_other_stop_at_one { 1. - min_gap } else { 1. };

				let clamped = new_pos.clamp(left_bound, right_bound);
				self.gradient.position[s] = clamped;
				let new_position = self.gradient.position[s];
				let new_color = self.gradient.color[s];

				self.gradient.sort();
				if let Some(new_index) = self.gradient.iter().position(|s| s.position == new_position && s.color == new_color) {
					self.dragging = GradientDragTarget::Stop(new_index);
				}
			}
			GradientDragTarget::Midpoint(midpoint_index) => {
				let document_to_viewport = snap_data.document.metadata().document_to_viewport;

				let (viewport_start, viewport_end) = self.viewport_handle_positions();

				let line_length = viewport_start.distance(viewport_end);
				if line_length < f64::EPSILON {
					self.render_gradient(responses);
					return;
				}

				let (document_start, document_end) = (
					document_to_viewport.inverse().transform_point2(viewport_start),
					document_to_viewport.inverse().transform_point2(viewport_end),
				);

				let constraint = SnapConstraint::Line {
					origin: document_start,
					direction: document_end - document_start,
				};

				let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
				let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);

				let snapped = snap_manager.constrained_snap(&snap_data, &point_candidate, constraint, SnapTypeConfiguration::default());

				let projected_mouse_document = if snapped.is_snapped() {
					snapped.snapped_point_document
				} else {
					constraint.projection(document_mouse)
				};
				let projected_mouse = document_to_viewport.transform_point2(projected_mouse_document);
				snap_manager.update_indicator(snapped);

				// Calculate the position along the full gradient (0-1)
				let full_pos = ((viewport_end - viewport_start).angle_to(projected_mouse - viewport_start)).cos() * viewport_start.distance(projected_mouse) / line_length;

				if !full_pos.is_finite() {
					self.render_gradient(responses);
					return;
				}

				// Convert to a midpoint ratio within the interval between the two surrounding stops
				let left_stop = self.gradient.position[midpoint_index];
				let right_stop = self.gradient.position[midpoint_index + 1];
				let range = right_stop - left_stop;
				if range > 0. {
					let midpoint_ratio = ((full_pos - left_stop) / range).clamp(GRADIENT_MIDPOINT_MIN, GRADIENT_MIDPOINT_MAX);
					self.gradient.midpoint[midpoint_index] = midpoint_ratio;
				}
			}
		}
		self.render_gradient(responses);
	}

	/// Update the layer fill to the current gradient
	pub fn render_gradient(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(layer) = self.layer {
			if self.is_gradient_chain {
				dispatch_gradient_chain_writes(layer, &self.gradient, self.appearance, responses);
			} else {
				responses.add(GraphOperationMessage::FillGradientSet {
					layer,
					gradient: self.gradient.clone(),
					gradient_type: self.appearance.gradient_type,
					spread_method: self.appearance.spread_method,
					transform: self.appearance.transform,
				});
			}
		}
	}

	fn unit_to_viewport_transform(&self) -> DAffine2 {
		self.gradient_space_transform * self.appearance.transform
	}

	fn viewport_handle_positions(&self) -> (DVec2, DVec2) {
		gradient_handle_positions(self.unit_to_viewport_transform())
	}
}

/// Send the four per-attribute graph operations that mirror the in-memory `Gradient` onto the chain feeding the layer.
fn dispatch_gradient_chain_writes(layer: LayerNodeIdentifier, gradient: &GradientStops, appearance: GradientAppearance, responses: &mut VecDeque<Message>) {
	responses.add(GraphOperationMessage::GradientStopsSet { layer, stops: gradient.clone() });
	responses.add(GraphOperationMessage::GradientTransformSet {
		layer,
		transform: appearance.transform,
	});
	responses.add(GraphOperationMessage::GradientTypeSet {
		layer,
		gradient_type: appearance.gradient_type,
	});
	responses.add(GraphOperationMessage::GradientSpreadMethodSet {
		layer,
		spread_method: appearance.spread_method,
	});
}

impl GradientTool {
	/// Get the gradient type of the selected gradient (if it exists)
	pub fn selected_gradient(&self) -> Option<GradientType> {
		self.data.selected_gradient.as_ref().map(|selected| selected.appearance.gradient_type)
	}
}

impl ToolTransition for GradientTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(GradientToolMessage::Abort.into()),
			selection_changed: Some(GradientToolMessage::SelectionChanged.into()),
			working_color_changed: Some(GradientToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|context| GradientToolMessage::Overlays { context }.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct GradientToolData {
	selected_gradient: Option<SelectedGradient>,
	snap_manager: SnapManager,
	drag_start: DVec2,
	/// The pointer-down position before snapping (document space), used to detect whether the mouse moved between the press and a double-click.
	drag_start_unsnapped: DVec2,
	auto_panning: AutoPanning,
	auto_pan_shift: DVec2,
	gradient_angle: f64,
	has_selected_gradient: bool,
	/// Cached stops of the currently selected layer's gradient, mirrored into the control-bar widget.
	/// Independent of any in-progress drag (which uses `selected_gradient`) so it stays current after selection changes too.
	current_gradient_stops: Option<GradientStops>,
	/// User-customized default gradient stop colors: used when nothing that has a gradient is selected.
	/// `None` means to follow the working colors.
	/// Cleared on tool deactivation so each fresh activation starts from the working colors again.
	default_gradient_stops: Option<GradientStops>,
	/// Cached viewport-space orientation (true = predominantly rightward) of the selected gradient line.
	/// Used to refresh the control bar's "Reverse Direction" icon only when the line's apparent direction flips.
	gradient_orientation_rightward: bool,
	/// Cached working colors, mirrored from `DocumentToolData` via the `WorkingColorChanged` event, used as the default gradient colors.
	primary_color: Color,
	secondary_color: Color,
	color_picker_editing_color_stop: Option<usize>,
	color_picker_transaction_open: bool,
}

impl Fsm for GradientToolFsmState {
	type ToolData = GradientToolData;
	type ToolOptions = GradientOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document,
			global_tool_data,
			input,
			viewport,
			..
		} = tool_action_data;

		let ToolMessage::Gradient(event) = event else { return self };
		match (self, event) {
			(_, GradientToolMessage::Overlays { context: mut overlay_context }) => {
				let selected = tool_data.selected_gradient.as_ref();
				let mouse = input.mouse.position;

				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some((gradient, appearance, _source)) = resolve_gradient(layer, &document.network_interface) else {
						continue;
					};
					let unit_to_viewport = gradient_space_transform(layer, document) * appearance.transform;
					let dragging = selected
						.filter(|selected| selected.layer.is_some_and(|selected_layer| selected_layer == layer))
						.map(|selected| selected.dragging);

					let gradient = if matches!(self, GradientToolFsmState::Drawing { .. })
						&& dragging.is_some()
						&& let Some(selected_gradient) = selected.filter(|s| s.layer == Some(layer))
					{
						&selected_gradient.gradient
					} else {
						&gradient
					};

					let (start, end) = (unit_to_viewport.transform_point2(DVec2::ZERO), unit_to_viewport.transform_point2(DVec2::X));

					fn color_to_hex(color: graphene_std::Color) -> String {
						SRGBA8::from(color).to_css_hex()
					}

					let start_hex = gradient.color.first().map(|&c| color_to_hex(c)).unwrap_or(String::from(COLOR_OVERLAY_BLUE));
					let end_hex = gradient.color.last().map(|&c| color_to_hex(c)).unwrap_or(String::from(COLOR_OVERLAY_BLUE));

					// Check if the first/last stops are at position ~0/~1 (rendered as the endpoint dots rather than as separate stops)
					let first_at_start = gradient.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.);
					let last_at_end = gradient.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.);

					overlay_context.line(start, end, None, None);

					// Determine which stop is selected (being dragged) and hovered (closest to mouse)
					// so they can be drawn last to appear on top of other overlapping stops
					let selected_stop_id: Option<StopId> = match dragging {
						Some(GradientDragTarget::Start) => Some(StopId::Start),
						Some(GradientDragTarget::End) => Some(StopId::End),
						Some(GradientDragTarget::Stop(0)) if first_at_start => Some(StopId::Start),
						Some(GradientDragTarget::Stop(i)) if last_at_end && i == gradient.len() - 1 => Some(StopId::End),
						Some(GradientDragTarget::Stop(i)) => Some(StopId::Middle(i)),
						_ => None,
					};
					let stop_tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);
					let hovered_stop_id: Option<StopId> = if !matches!(self, GradientToolFsmState::Drawing { .. }) {
						// Find the closest stop to the mouse (matching the click detection logic)
						let mut best: Option<(f64, StopId)> = None;
						let mut check = |dist_sq: f64, id: StopId| {
							if dist_sq < stop_tolerance && best.as_ref().is_none_or(|&(d, _)| dist_sq < d) {
								best = Some((dist_sq, id));
							}
						};
						check(start.distance_squared(mouse), StopId::Start);
						check(end.distance_squared(mouse), StopId::End);
						for (index, stop) in gradient.iter().enumerate() {
							if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
								continue;
							}
							check(start.lerp(end, stop.position).distance_squared(mouse), StopId::Middle(index));
						}
						best.map(|(_, id)| id)
					} else {
						None
					};

					// Draw order: regular stops first, then selected, then hovered (so hovered appears on top)
					let is_deferred = |id: StopId| -> bool { Some(id) == selected_stop_id || Some(id) == hovered_stop_id };
					let emphasis_for = |id: StopId| -> GizmoEmphasis {
						if Some(id) == selected_stop_id {
							GizmoEmphasis::Active
						} else if Some(id) == hovered_stop_id {
							GizmoEmphasis::Hovered
						} else {
							GizmoEmphasis::Regular
						}
					};
					let mut draw_stop = |id: StopId, emphasis: GizmoEmphasis| match id {
						StopId::Start => overlay_context.gradient_color_stop(start, emphasis, &start_hex, !first_at_start),
						StopId::End => overlay_context.gradient_color_stop(end, emphasis, &end_hex, !last_at_end),
						StopId::Middle(i) => {
							if let Some(stop) = gradient.iter().nth(i) {
								overlay_context.gradient_color_stop(start.lerp(end, stop.position), emphasis, &color_to_hex(stop.color), false);
							}
						}
					};

					// Draw regular (non-deferred) stops
					if !is_deferred(StopId::Start) {
						draw_stop(StopId::Start, emphasis_for(StopId::Start));
					}
					if !is_deferred(StopId::End) {
						draw_stop(StopId::End, emphasis_for(StopId::End));
					}
					for (index, stop) in gradient.iter().enumerate() {
						if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
							continue;
						}
						let id = StopId::Middle(index);
						if !is_deferred(id) {
							draw_stop(id, emphasis_for(id));
						}
					}

					// Draw selected stop (if not also hovered)
					if let Some(selected_id) = selected_stop_id
						&& Some(selected_id) != hovered_stop_id
					{
						draw_stop(selected_id, GizmoEmphasis::Active);
					}

					// Draw hovered stop last (on top of everything)
					if let Some(hov_id) = hovered_stop_id {
						let emphasis = if Some(hov_id) == selected_stop_id { GizmoEmphasis::Active } else { GizmoEmphasis::Hovered };
						draw_stop(hov_id, emphasis);
					}

					// Draw midpoint diamonds between adjacent stops (hidden when stops are too close in viewport space)
					let line_angle = (end - start).to_angle();
					let line_length = start.distance(end);
					let midpoint_tolerance = GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2);
					for i in 0..gradient.position.len().saturating_sub(1) {
						let left = gradient.position[i];
						let right = gradient.position[i + 1];

						if midpoint_hidden_by_proximity(left, right, line_length) {
							continue;
						}

						let midpoint_pos = left + gradient.midpoint[i] * (right - left);
						let midpoint_viewport = start.lerp(end, midpoint_pos);

						let emphasis = if dragging == Some(GradientDragTarget::Midpoint(i)) {
							GizmoEmphasis::Active
						} else if !matches!(self, GradientToolFsmState::Drawing { .. }) && midpoint_viewport.distance_squared(mouse) < midpoint_tolerance {
							GizmoEmphasis::Hovered
						} else {
							GizmoEmphasis::Regular
						};
						overlay_context.gradient_midpoint(midpoint_viewport, emphasis, line_angle);
					}

					if !matches!(self, GradientToolFsmState::Drawing { .. })
						&& calculate_insertion(start, end, gradient, mouse).is_some()
						&& let Some(dir) = (end - start).try_normalize()
					{
						let perp = dir.perp();

						// Snap the insertion point along the gradient line
						let document_to_viewport = document.metadata().document_to_viewport;

						let (document_start, document_end) = (document_to_viewport.inverse().transform_point2(start), document_to_viewport.inverse().transform_point2(end));
						let constraint = SnapConstraint::Line {
							origin: document_start,
							direction: document_end - document_start,
						};

						let document_mouse = document_to_viewport.inverse().transform_point2(mouse);
						let point_candidate = SnapCandidatePoint::gradient_handle(document_mouse);

						let snap_data = SnapData::new(document, input, viewport);
						let snapped = tool_data.snap_manager.constrained_snap(&snap_data, &point_candidate, constraint, SnapTypeConfiguration::default());

						let snapped_point = if snapped.is_snapped() {
							document_to_viewport.transform_point2(snapped.snapped_point_document)
						} else {
							let projected = constraint.projection(document_mouse);
							document_to_viewport.transform_point2(projected)
						};

						overlay_context.line(
							snapped_point - perp * SEGMENT_OVERLAY_SIZE,
							snapped_point + perp * SEGMENT_OVERLAY_SIZE,
							Some(COLOR_OVERLAY_BLUE),
							Some(1.),
						);
					}
				}

				let snap_data = SnapData::new(document, input, viewport);
				tool_data.snap_manager.draw_overlays(snap_data, &mut overlay_context);

				// Update color picker position if active (keeps it anchored to the stop during pan/zoom)
				if let Some(stop_index) = tool_data.color_picker_editing_color_stop
					&& let Some(selected_gradient) = tool_data.selected_gradient.as_ref()
					&& let Some(layer) = selected_gradient.layer
				{
					// The gradient space transform has be recalculated as the saved transform in SelectedGradient may become stale by panning/zooming during the rendering of the overlay.
					let transform = gradient_space_transform(layer, document) * selected_gradient.appearance.transform;
					let gradient = &selected_gradient.gradient;
					if stop_index < gradient.position.len() {
						let color = gradient.color[stop_index];
						let position = gradient.position[stop_index];
						let start = transform.transform_point2(DVec2::ZERO);
						let end = transform.transform_point2(DVec2::X);
						let position = start.lerp(end, position).into();
						responses.add(FrontendMessage::UpdateGradientStopColorPickerPosition { color: color.into(), position });
					}
				}

				self
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::SelectionChanged) => {
				if tool_data.color_picker_editing_color_stop.is_some() {
					if tool_data.color_picker_transaction_open {
						responses.add(DocumentMessage::EndTransaction);
						tool_data.color_picker_transaction_open = false;
					}
					tool_data.color_picker_editing_color_stop = None;
				}
				tool_data.selected_gradient = None;
				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				}
			}
			(_, GradientToolMessage::DoubleClick) => {
				// Only reset if the mouse hasn't moved so we don't trigger from a click-then-click-and-drag being reported as a double-click.
				// Compared against the unsnapped press position so a snap point near the stop doesn't make a stationary mouse look moved.
				let drag_start_viewport = document.metadata().document_to_viewport.transform_point2(tool_data.drag_start_unsnapped);
				if input.mouse.position.distance(drag_start_viewport) <= DRAG_THRESHOLD
					&& let Some(selected_gradient) = &mut tool_data.selected_gradient
				{
					match selected_gradient.dragging {
						GradientDragTarget::Midpoint(index) => {
							selected_gradient.gradient.midpoint[index] = 0.5;
							selected_gradient.render_gradient(responses);
							responses.add(PropertiesPanelMessage::Refresh);
						}
						GradientDragTarget::Start | GradientDragTarget::End | GradientDragTarget::Stop(_) => {
							// Find the stop index from the drag target
							let stop_index = match selected_gradient.dragging {
								GradientDragTarget::Stop(i) => Some(i),
								GradientDragTarget::Start => selected_gradient.gradient.position.iter().position(|&p| p.abs() < f64::EPSILON * 1000.),
								GradientDragTarget::End => selected_gradient.gradient.position.iter().position(|&p| (1. - p).abs() < f64::EPSILON * 1000.),
								_ => None,
							};
							if let Some(stop_index) = stop_index
								&& stop_index < selected_gradient.gradient.color.len()
							{
								// Dismiss any existing color picker first
								if tool_data.color_picker_editing_color_stop.is_some() && tool_data.color_picker_transaction_open {
									responses.add(DocumentMessage::EndTransaction);
									tool_data.color_picker_transaction_open = false;
								}

								let stop_pos = selected_gradient.gradient.position[stop_index];
								let (start, end) = selected_gradient.viewport_handle_positions();
								let viewport_pos = start.lerp(end, stop_pos);
								let position = viewport_pos.into();
								let color = selected_gradient.gradient.color[stop_index];
								tool_data.color_picker_editing_color_stop = Some(stop_index);
								responses.add(FrontendMessage::UpdateGradientStopColorPickerPosition { color: color.into(), position });
							}
						}
						_ => {}
					}
				}
				self
			}
			(state, GradientToolMessage::DeleteStop) => {
				let ready_default = GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				};

				let Some(selected_gradient) = &mut tool_data.selected_gradient else {
					return ready_default;
				};

				// Skip if invalid gradient
				if selected_gradient.gradient.len() < 2 {
					return ready_default;
				}

				// If we're in the middle of a drag, abort it first and revert to the initial gradient
				if matches!(state, GradientToolFsmState::Drawing { .. }) {
					selected_gradient.gradient = selected_gradient.initial_gradient.clone();
					selected_gradient.render_gradient(responses);
					responses.add(DocumentMessage::AbortTransaction);
					tool_data.snap_manager.cleanup(responses);
				}

				responses.add(DocumentMessage::StartTransaction);

				// Remove the selected point
				match selected_gradient.dragging {
					GradientDragTarget::Start => {
						// Only delete if there's a real color stop at position ~0 (not the endpoint of the line which isn't itself a color stop)
						if selected_gradient.gradient.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.) {
							selected_gradient.gradient.remove(0);
						} else {
							responses.add(DocumentMessage::AbortTransaction);
							return ready_default;
						}
					}
					GradientDragTarget::End => {
						// Only delete if there's a real color stop at position ~1 (not the endpoint of the line which isn't itself a color stop)
						if selected_gradient.gradient.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.) {
							let _ = selected_gradient.gradient.pop();
						} else {
							responses.add(DocumentMessage::AbortTransaction);
							return ready_default;
						}
					}
					GradientDragTarget::New => {
						responses.add(DocumentMessage::AbortTransaction);
						return ready_default;
					}
					GradientDragTarget::Stop(index) => {
						selected_gradient.gradient.remove(index);
					}
					GradientDragTarget::Midpoint(index) => {
						selected_gradient.gradient.midpoint[index] = 0.5;
						selected_gradient.render_gradient(responses);

						responses.add(DocumentMessage::CommitTransaction);
						responses.add(PropertiesPanelMessage::Refresh);

						return ready_default;
					}
				};

				// The gradient has only one point and so should become a fill
				if selected_gradient.gradient.len() == 1 {
					if selected_gradient.is_gradient_chain {
						selected_gradient.render_gradient(responses);
					} else if let Some(layer) = selected_gradient.layer {
						responses.add(GraphOperationMessage::FillColorSet {
							layer,
							color: Some(selected_gradient.gradient.color[0]),
						});
					}
					responses.add(DocumentMessage::CommitTransaction);
					responses.add(PropertiesPanelMessage::Refresh);
					return ready_default;
				}

				// Find the minimum and maximum positions
				let min_position = selected_gradient.gradient.position.iter().copied().reduce(f64::min).expect("No min");
				let max_position = selected_gradient.gradient.position.iter().copied().reduce(f64::max).expect("No max");

				let gradient_transform = selected_gradient.appearance.transform;
				let (local_start, local_end) = (gradient_transform.transform_point2(DVec2::ZERO), gradient_transform.transform_point2(DVec2::X));
				selected_gradient.appearance.transform = build_transform_with_y_preservation(gradient_transform, local_start.lerp(local_end, min_position), local_start.lerp(local_end, max_position));

				// Remap the positions
				for position in selected_gradient.gradient.position.iter_mut() {
					*position = (*position - min_position) / (max_position - min_position);
				}

				// Render the new gradient
				selected_gradient.render_gradient(responses);
				responses.add(DocumentMessage::CommitTransaction);
				responses.add(PropertiesPanelMessage::Refresh);
				tool_data.selected_gradient = None;

				ready_default
			}
			(_, GradientToolMessage::InsertStop) => {
				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some((mut gradient, appearance, source)) = resolve_gradient(layer, &document.network_interface) else {
						continue;
					};
					// TODO: This transform is incorrect. I think this is since it is based on the Footprint which has not been updated yet
					let unit_to_viewport = gradient_space_transform(layer, document) * appearance.transform;
					let mouse = input.mouse.position;
					let (start, end) = gradient_handle_positions(unit_to_viewport);

					// Compute the distance from the mouse to the gradient line in viewport space
					let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();

					// If click is on the line then insert point
					if distance < (SELECTION_THRESHOLD * 2.) {
						// Try and insert the new stop
						if let Some(index) = insert_stop_at_point(&mut gradient, mouse, unit_to_viewport) {
							responses.add(DocumentMessage::StartTransaction);

							let mut selected_gradient = SelectedGradient::new(gradient, appearance, source, layer, document);

							// Select the new point
							selected_gradient.dragging = GradientDragTarget::Stop(index);

							// Update the layer fill
							selected_gradient.render_gradient(responses);

							tool_data.selected_gradient = Some(selected_gradient);
							responses.add(DocumentMessage::CommitTransaction);
							break;
						}
					}
				}

				self
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::PointerDown) => {
				let document_to_viewport = document.metadata().document_to_viewport;

				let mut mouse = input.mouse.position;

				let snap_data = SnapData::new(document, input, viewport);
				let point = SnapCandidatePoint::gradient_handle(document_to_viewport.inverse().transform_point2(mouse));
				let snapped = tool_data.snap_manager.free_snap(&snap_data, &point, SnapTypeConfiguration::default());

				if snapped.is_snapped() {
					mouse = document_to_viewport.transform_point2(snapped.snapped_point_document);
				}

				tool_data.drag_start = document_to_viewport.inverse().transform_point2(mouse);
				tool_data.drag_start_unsnapped = point.document_point;
				tool_data.auto_pan_shift = DVec2::ZERO;
				let tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);

				let mut drag_hint: Option<GradientDragHintState> = None;
				let mut transaction_started = false;
				for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
					let Some((gradient, appearance, source)) = resolve_gradient(layer, &document.network_interface) else {
						continue;
					};
					let gradient_space_transform = gradient_space_transform(layer, document);
					let unit_to_viewport = gradient_space_transform * appearance.transform;
					let is_gradient_chain = source == GradientSource::Chain;
					let (start, end) = gradient_handle_positions(unit_to_viewport);

					// Check for dragging a midpoint diamond
					if drag_hint.is_none() {
						let line_length = start.distance(end);
						let midpoint_tolerance = GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2);
						for i in 0..gradient.position.len().saturating_sub(1) {
							let left = gradient.position[i];
							let right = gradient.position[i + 1];

							if midpoint_hidden_by_proximity(left, right, line_length) {
								continue;
							}

							let midpoint_pos = left + gradient.midpoint[i] * (right - left);
							let midpoint_viewport = start.lerp(end, midpoint_pos);

							if midpoint_viewport.distance_squared(mouse) < midpoint_tolerance {
								let resettable = midpoint_is_resettable(gradient.midpoint[i]);
								drag_hint = Some(GradientDragHintState::Midpoint { resettable });

								tool_data.selected_gradient = Some(SelectedGradient {
									layer: Some(layer),
									gradient_space_transform,
									gradient: gradient.clone(),
									appearance,
									initial_gradient_transform: appearance.transform,
									dragging: GradientDragTarget::Midpoint(i),
									initial_gradient: gradient.clone(),
									is_gradient_chain,
								});

								break;
							}
						}
					}

					// Check for dragging the closest stop to the mouse pointer
					if drag_hint.is_none() {
						let mut best: Option<(f64, usize)> = None;
						for (index, stop) in gradient.iter().enumerate() {
							let pos = start.lerp(end, stop.position);
							let dist_sq = pos.distance_squared(mouse);
							if dist_sq < tolerance && best.as_ref().is_none_or(|&(best_dist, _)| dist_sq < best_dist) {
								best = Some((dist_sq, index));
							}
						}
						if let Some((_, index)) = best {
							let stop_position = gradient.position[index];
							// Stops at position 0 or 1 are locked endpoints: dragging moves the
							// gradient line endpoint geometry (start/end) instead of stop position
							let drag_target = if stop_position.abs() < f64::EPSILON * 1000. {
								GradientDragTarget::Start
							} else if (1. - stop_position).abs() < f64::EPSILON * 1000. {
								GradientDragTarget::End
							} else {
								GradientDragTarget::Stop(index)
							};

							drag_hint = Some(match drag_target {
								GradientDragTarget::Start | GradientDragTarget::End => GradientDragHintState::EndStop,
								_ => GradientDragHintState::Stop,
							});

							tool_data.selected_gradient = Some(SelectedGradient {
								layer: Some(layer),
								dragging: drag_target,
								gradient_space_transform,
								gradient: gradient.clone(),
								appearance,
								initial_gradient: gradient.clone(),
								initial_gradient_transform: appearance.transform,
								is_gradient_chain,
							});
						}
					}

					// Check dragging start or end handle
					if drag_hint.is_none() {
						for (pos, dragging_target) in [(start, GradientDragTarget::Start), (end, GradientDragTarget::End)] {
							if pos.distance_squared(mouse) < tolerance {
								drag_hint = Some(GradientDragHintState::Endpoint);
								tool_data.selected_gradient = Some(SelectedGradient {
									layer: Some(layer),
									dragging: dragging_target,
									gradient_space_transform,
									gradient: gradient.clone(),
									appearance,
									initial_gradient: gradient.clone(),
									initial_gradient_transform: appearance.transform,
									is_gradient_chain,
								})
							}
						}
					}

					// Insert stop if clicking on line
					if drag_hint.is_none() {
						let distance = (end - start).angle_to(mouse - start).sin() * (mouse - start).length();
						let projection = ((end - start).angle_to(mouse - start)).cos() * start.distance(mouse) / start.distance(end);

						if distance.abs() < SEGMENT_INSERTION_DISTANCE && (0. ..=1.).contains(&projection) {
							let mut new_gradient = gradient.clone();
							if let Some(index) = insert_stop_at_point(&mut new_gradient, mouse, unit_to_viewport) {
								responses.add(DocumentMessage::StartTransaction);
								transaction_started = true;

								let mut selected_gradient = SelectedGradient::new(new_gradient, appearance, source, layer, document);
								selected_gradient.dragging = GradientDragTarget::Stop(index);
								// No offset when inserting a new stop, it should be exactly under the mouse
								selected_gradient.render_gradient(responses);
								tool_data.selected_gradient = Some(selected_gradient);
								drag_hint = Some(GradientDragHintState::Stop);
							}
						}
					}
				}

				// Initialize `gradient_angle` from the existing gradient so Ctrl (lock angle) works from the first mouse move
				if let Some(selected_gradient) = &tool_data.selected_gradient {
					let (vp_start, vp_end) = selected_gradient.viewport_handle_positions();
					let delta = match selected_gradient.dragging {
						// When dragging End, the fixed point is start and the mouse begins at end
						GradientDragTarget::End => vp_start - vp_end,
						// When dragging Start, the fixed point is end and the mouse begins at start
						GradientDragTarget::Start => vp_end - vp_start,
						_ => vp_start - vp_end,
					};
					tool_data.gradient_angle = -delta.angle_to(DVec2::X);
				}

				let gradient_state = if let Some(hint) = drag_hint {
					GradientToolFsmState::Drawing { drag_hint: hint }
				} else {
					let document_mouse = document.metadata().document_to_viewport.inverse().transform_point2(mouse);
					// List-based gradients render no geometry, so a click on empty canvas yields no layer.
					// Fall back to a selected gradient list layer so the user can drag a fresh gradient line anywhere.
					let selected_layer = document.click_based_on_position(document_mouse).or_else(|| {
						document
							.network_interface
							.selected_nodes()
							.selected_visible_layers(&document.network_interface)
							.find(|&layer| get_gradient_stops(layer, &document.network_interface).is_some())
					});

					// Apply the gradient to the selected layer
					if let Some(layer) = selected_layer {
						// Add check for raster layer
						if NodeGraphLayer::is_raster_layer(layer, &mut document.network_interface) {
							return GradientToolFsmState::Ready {
								hovering: GradientHoverTarget::None,
								selected: GradientSelectedTarget::None,
							};
						}
						if !document.network_interface.selected_nodes().selected_layers_contains(layer, document.metadata()) {
							let nodes = vec![layer.to_node()];

							responses.add(NodeGraphMessage::SelectedNodesSet { nodes });
						}

						let (gradient, appearance, source) = match resolve_gradient(layer, &document.network_interface) {
							// Use the already existing gradient if it exists
							Some(gradient) => gradient,
							// Generate a new gradient running primary → secondary so the default working colors
							// (primary = black, secondary = white) produce the expected black-to-white gradient
							None => (
								GradientStops::new([
									GradientStop {
										position: 0.,
										midpoint: 0.5,
										color: global_tool_data.primary_color,
									},
									GradientStop {
										position: 1.,
										midpoint: 0.5,
										color: global_tool_data.secondary_color,
									},
								]),
								GradientAppearance {
									transform: DAffine2::IDENTITY,
									gradient_type: tool_options.gradient_type,
									spread_method: tool_options.spread_method,
								},
								GradientSource::Direct,
							),
						};
						let mut selected_gradient = SelectedGradient::new(gradient, appearance, source, layer, document);
						selected_gradient.dragging = GradientDragTarget::New;

						tool_data.selected_gradient = Some(selected_gradient);

						GradientToolFsmState::Drawing {
							drag_hint: GradientDragHintState::NewGradient,
						}
					} else {
						GradientToolFsmState::Ready {
							hovering: GradientHoverTarget::None,
							selected: GradientSelectedTarget::None,
						}
					}
				};

				if matches!(gradient_state, GradientToolFsmState::Drawing { .. }) && !transaction_started {
					responses.add(DocumentMessage::StartTransaction);
				}

				responses.add(OverlaysMessage::Draw);

				gradient_state
			}
			(GradientToolFsmState::Drawing { drag_hint }, GradientToolMessage::PointerMove { constrain_axis, lock_angle }) => {
				if let Some(selected_gradient) = &mut tool_data.selected_gradient {
					let mouse = input.mouse.position;
					let snap_data = SnapData::new(document, input, viewport);

					// Recompute the gradient-to-viewport transform fresh each frame so zoom/pan mid-drag works correctly
					if let Some(layer) = selected_gradient.layer {
						selected_gradient.gradient_space_transform = gradient_space_transform(layer, document);
						selected_gradient.gradient_space_transform.translation += tool_data.auto_pan_shift;
					}

					// Convert drag_start from document space to effective viewport space
					let document_to_viewport = document.metadata().document_to_viewport;
					let drag_start_viewport = document_to_viewport.transform_point2(tool_data.drag_start) + tool_data.auto_pan_shift;
					tool_data.auto_pan_shift = DVec2::ZERO;

					selected_gradient.update_gradient(
						mouse,
						responses,
						input.keyboard.get(constrain_axis as usize),
						input.keyboard.get(lock_angle as usize),
						selected_gradient.appearance.gradient_type,
						drag_start_viewport,
						snap_data,
						&mut tool_data.snap_manager,
						&mut tool_data.gradient_angle,
					);
				}

				// Auto-panning
				let messages = [
					GradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }.into(),
					GradientToolMessage::PointerMove { constrain_axis, lock_angle }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, viewport, &messages, responses);

				responses.add(OverlaysMessage::Draw);

				GradientToolFsmState::Drawing { drag_hint }
			}
			(GradientToolFsmState::Drawing { drag_hint }, GradientToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				if let Some(shift) = tool_data.auto_panning.shift_viewport(input, viewport, responses) {
					tool_data.auto_pan_shift += shift;
				}

				GradientToolFsmState::Drawing { drag_hint }
			}
			(state, GradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }) => {
				// Auto-panning
				let messages = [
					GradientToolMessage::PointerOutsideViewport { constrain_axis, lock_angle }.into(),
					GradientToolMessage::PointerMove { constrain_axis, lock_angle }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(GradientToolFsmState::Drawing { .. }, GradientToolMessage::PointerUp) => {
				responses.add(DocumentMessage::EndTransaction);
				tool_data.snap_manager.cleanup(responses);

				// Clear the selection if we were dragging an endpoint of the gradient which isn't a stop
				if tool_data.selected_gradient.as_ref().is_some_and(|s| match s.dragging {
					GradientDragTarget::Start => !s.gradient.position.first().is_some_and(|&p| p.abs() < f64::EPSILON * 1000.),
					GradientDragTarget::End => !s.gradient.position.last().is_some_and(|&p| (1. - p).abs() < f64::EPSILON * 1000.),
					_ => false,
				}) {
					tool_data.selected_gradient = None;
				}

				let selected = compute_selected_target(tool_data);
				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected,
				}
			}
			(GradientToolFsmState::Ready { .. }, GradientToolMessage::PointerMove { .. }) => {
				let mouse = input.mouse.position;
				let hovering = detect_hover_target(mouse, document);
				let selected = compute_selected_target(tool_data);

				let snap_data = SnapData::new(document, input, viewport);
				tool_data.snap_manager.preview_draw_gradient(&snap_data, mouse);

				responses.add(OverlaysMessage::Draw);
				GradientToolFsmState::Ready { hovering, selected }
			}

			(GradientToolFsmState::Drawing { .. }, GradientToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				tool_data.snap_manager.cleanup(responses);
				tool_data.selected_gradient = None;
				responses.add(OverlaysMessage::Draw);

				dismiss_color_stop_color_picker(tool_data, responses);

				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				}
			}
			(_, GradientToolMessage::Abort) => {
				dismiss_color_stop_color_picker(tool_data, responses);
				// Clear the tool-default gradient override so re-activating the tool starts fresh from the working colors
				tool_data.default_gradient_stops = None;

				GradientToolFsmState::Ready {
					hovering: GradientHoverTarget::None,
					selected: GradientSelectedTarget::None,
				}
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			GradientToolFsmState::Ready { hovering, selected } => {
				let mut groups = Vec::new();

				// Primary hints based on hover target
				match hovering {
					GradientHoverTarget::None => {
						groups.push(HintGroup(vec![
							HintInfo::mouse(MouseMotion::LmbDrag, "Draw Gradient"),
							HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
							HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
						]));
					}
					GradientHoverTarget::InsertionPoint => {
						groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Insert Color Stop")]));
					}
					GradientHoverTarget::Stop => {
						groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Color Stop")]));
					}
					GradientHoverTarget::Endpoint => {
						groups.push(HintGroup(vec![
							HintInfo::mouse(MouseMotion::LmbDrag, "Move Gradient End"),
							HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
							HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
						]));
					}
					GradientHoverTarget::Midpoint { resettable } => {
						groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDrag, "Move Midpoint")]));
						if *resettable {
							groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::LmbDouble, "Reset Midpoint")]));
						}
					}
				}

				// Delete/reset hint based on selection
				match selected {
					GradientSelectedTarget::Stop => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Color Stop")]));
					}
					GradientSelectedTarget::Midpoint { resettable: true } => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Reset Midpoint")]));
					}
					_ => {}
				}

				HintData(groups)
			}
			GradientToolFsmState::Drawing { drag_hint } => {
				let mut groups = Vec::new();

				// Abort hints
				groups.push(HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]));

				// Angle constraint hint (only for endpoint/end color stop/new gradient dragging)
				if matches!(drag_hint, GradientDragHintState::NewGradient | GradientDragHintState::Endpoint | GradientDragHintState::EndStop) {
					groups.push(HintGroup(vec![HintInfo::keys([Key::Shift], "15° Increments"), HintInfo::keys([Key::Control], "Lock Angle")]));
				}

				// Delete/reset hint while dragging
				match drag_hint {
					GradientDragHintState::EndStop | GradientDragHintState::Stop => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Delete Color Stop")]));
					}
					GradientDragHintState::Midpoint { resettable: true } => {
						groups.push(HintGroup(vec![HintInfo::keys([Key::Backspace], "Reset Midpoint")]));
					}
					_ => {}
				}

				HintData(groups)
			}
		};

		hint_data.send_layout(responses);
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

fn insert_stop_at_point(gradient: &mut GradientStops, point: DVec2, unit_to_viewport: DAffine2) -> Option<usize> {
	let (start, end) = gradient_handle_positions(unit_to_viewport);
	let t = ((end - start).angle_to(point - start)).cos() * start.distance(point) / start.distance(end);
	(0. ..=1.).contains(&t).then(|| gradient.insert_stop(t))
}

fn dismiss_color_stop_color_picker(tool_data: &mut GradientToolData, responses: &mut VecDeque<Message>) {
	if tool_data.color_picker_editing_color_stop.is_some() {
		if tool_data.color_picker_transaction_open {
			responses.add(DocumentMessage::EndTransaction);
			tool_data.color_picker_transaction_open = false;
		}
		tool_data.color_picker_editing_color_stop = None;
	}
}

fn detect_hover_target(mouse: DVec2, document: &DocumentMessageHandler) -> GradientHoverTarget {
	let stop_tolerance = (MANIPULATOR_GROUP_MARKER_SIZE * 2.).powi(2);
	let midpoint_tolerance = GRADIENT_MIDPOINT_DIAMOND_RADIUS.powi(2);

	for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
		let Some((gradient, appearance, _source)) = resolve_gradient(layer, &document.network_interface) else {
			continue;
		};
		let gradient_space_transform = gradient_space_transform(layer, document);
		let unit_to_viewport = gradient_space_transform * appearance.transform;
		let (start, end) = gradient_handle_positions(unit_to_viewport);
		let line_length = start.distance(end);

		// Check midpoint diamonds first (smaller hit area, higher priority)
		for i in 0..gradient.position.len().saturating_sub(1) {
			let left = gradient.position[i];
			let right = gradient.position[i + 1];
			if midpoint_hidden_by_proximity(left, right, line_length) {
				continue;
			}

			let midpoint_position = left + gradient.midpoint[i] * (right - left);
			let midpoint_viewport = start.lerp(end, midpoint_position);

			if midpoint_viewport.distance_squared(mouse) < midpoint_tolerance {
				let resettable = midpoint_is_resettable(gradient.midpoint[i]);
				return GradientHoverTarget::Midpoint { resettable };
			}
		}

		// Check stops
		for stop in gradient.iter() {
			let pos = start.lerp(end, stop.position);
			if pos.distance_squared(mouse) < stop_tolerance {
				return if stop.position.abs() < f64::EPSILON * 1000. || (1. - stop.position).abs() < f64::EPSILON * 1000. {
					GradientHoverTarget::Endpoint
				} else {
					GradientHoverTarget::Stop
				};
			}
		}

		// Check start/end handles (pure endpoints without stops)
		for endpoint_position in [start, end] {
			if endpoint_position.distance_squared(mouse) < stop_tolerance {
				return GradientHoverTarget::Endpoint;
			}
		}

		// Check insertion point on line
		if calculate_insertion(start, end, &gradient, mouse).is_some() {
			return GradientHoverTarget::InsertionPoint;
		}
	}

	GradientHoverTarget::None
}

fn compute_selected_target(tool_data: &GradientToolData) -> GradientSelectedTarget {
	let Some(selected_gradient) = &tool_data.selected_gradient else {
		return GradientSelectedTarget::None;
	};

	match selected_gradient.dragging {
		GradientDragTarget::Stop(_) | GradientDragTarget::Start | GradientDragTarget::End => GradientSelectedTarget::Stop,
		GradientDragTarget::Midpoint(i) => {
			let resettable = selected_gradient.gradient.midpoint.get(i).is_some_and(|&midpoint_value| midpoint_is_resettable(midpoint_value));
			GradientSelectedTarget::Midpoint { resettable }
		}
		GradientDragTarget::New => GradientSelectedTarget::None,
	}
}

fn apply_gradient_update(
	data: &mut GradientToolData,
	context: &mut ToolActionMessageContext,
	responses: &mut VecDeque<Message>,
	condition: impl Fn((&GradientStops, &GradientAppearance)) -> bool,
	update: impl Fn((&mut GradientStops, &mut GradientAppearance)),
) {
	let selected_layers: Vec<_> = context
		.document
		.network_interface
		.selected_nodes()
		.selected_visible_layers(&context.document.network_interface)
		.collect();

	let mut transaction_started = false;
	for layer in selected_layers {
		if NodeGraphLayer::is_raster_layer(layer, &mut context.document.network_interface) {
			continue;
		}

		if let Some((mut gradient, mut appearance, _)) = resolve_gradient(layer, &context.document.network_interface)
			&& condition((&gradient, &appearance))
		{
			if !transaction_started {
				responses.add(DocumentMessage::StartTransaction);
				transaction_started = true;
			}
			update((&mut gradient, &mut appearance));

			// Only check for the gradient list once we know we'll write back, since this is a graph traversal per layer
			if get_upstream_gradient_value_node_id(layer, &context.document.network_interface).is_some() {
				dispatch_gradient_chain_writes(layer, &gradient, appearance, responses);
			} else {
				responses.add(GraphOperationMessage::FillGradientSet {
					layer,
					gradient,
					gradient_type: appearance.gradient_type,
					spread_method: appearance.spread_method,
					transform: appearance.transform,
				});
			}
		}
	}

	if transaction_started {
		responses.add(DocumentMessage::EndTransaction);
	}
	if let Some(selected_gradient) = &mut data.selected_gradient
		&& let Some(layer) = selected_gradient.layer
		&& !NodeGraphLayer::is_raster_layer(layer, &mut context.document.network_interface)
	{
		update((&mut selected_gradient.gradient, &mut selected_gradient.appearance));
	}
	responses.add(PropertiesPanelMessage::Refresh);
	data.has_selected_gradient = has_gradient_on_selected_layers(context.document);
	responses.add(ToolMessage::RefreshToolOptions);
}

/// Set new gradient stops on every selected layer's gradient. Unlike `apply_gradient_update`, this doesn't open its own
/// transaction so it can be called repeatedly during a color picker drag and have all the changes coalesced into a
/// single undo entry by the surrounding 'on_commit' callback.
fn apply_stops_update(data: &mut GradientToolData, context: &mut ToolActionMessageContext, responses: &mut VecDeque<Message>, new_gradient: GradientStops) {
	let selected_layers: Vec<_> = context
		.document
		.network_interface
		.selected_nodes()
		.selected_visible_layers(&context.document.network_interface)
		.collect();

	let mut updated_any_layer = false;
	for layer in selected_layers {
		if NodeGraphLayer::is_raster_layer(layer, &mut context.document.network_interface) {
			continue;
		}

		if get_upstream_gradient_value_node_id(layer, &context.document.network_interface).is_some() {
			responses.add(GraphOperationMessage::GradientStopsSet { layer, stops: new_gradient.clone() });
			updated_any_layer = true;
		} else if let Some((_gradient, appearance, _source)) = resolve_gradient(layer, &context.document.network_interface) {
			responses.add(GraphOperationMessage::FillGradientSet {
				layer,
				gradient: new_gradient.clone(),
				gradient_type: appearance.gradient_type,
				spread_method: appearance.spread_method,
				transform: appearance.transform,
			});
			updated_any_layer = true;
		}
	}

	if let Some(selected_gradient) = &mut data.selected_gradient {
		selected_gradient.gradient = new_gradient.clone();
	}

	// When no selected layer had a gradient to update, the user is editing the tool's default gradient instead.
	// Save those stops so the widget keeps showing them until the tool is deactivated.
	if !updated_any_layer {
		data.default_gradient_stops = Some(new_gradient);
	}

	responses.add(PropertiesPanelMessage::Refresh);
	// Refresh the tool options so the swatch's `chosen_gradient` (precomputed CSS string) updates live as the user edits stops in the picker.
	responses.add(ToolMessage::RefreshToolOptions);
}

/// Find the first selected visible layer that has a gradient and return both the layer ID and its resolved gradient.
fn current_layer_and_gradient(document: &DocumentMessageHandler) -> (Option<LayerNodeIdentifier>, Option<(GradientStops, GradientAppearance)>) {
	for layer in document.network_interface.selected_nodes().selected_visible_layers(&document.network_interface) {
		if let Some((gradient, appearance, _source)) = resolve_gradient(layer, &document.network_interface) {
			return (Some(layer), Some((gradient, appearance)));
		}
	}
	(None, None)
}

fn get_gradient_on_selected_layer(document: &DocumentMessageHandler) -> Option<(GradientStops, GradientAppearance, GradientSource)> {
	document
		.network_interface
		.selected_nodes()
		.selected_visible_layers(&document.network_interface)
		.find_map(|layer| resolve_gradient(layer, &document.network_interface))
}

fn has_gradient_on_selected_layers(document: &DocumentMessageHandler) -> bool {
	get_gradient_on_selected_layer(document).is_some()
}

#[inline(always)]
fn midpoint_is_resettable(value: f64) -> bool {
	(value - 0.5).abs() >= f64::EPSILON * 1000.
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StopId {
	Start,
	End,
	Middle(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum GradientHoverTarget {
	#[default]
	None,
	InsertionPoint,
	Stop,
	Endpoint,
	Midpoint {
		resettable: bool,
	},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum GradientSelectedTarget {
	#[default]
	None,
	Stop,
	Midpoint {
		resettable: bool,
	},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum GradientDragHintState {
	#[default]
	NewGradient,
	Endpoint,
	EndStop,
	Stop,
	Midpoint {
		resettable: bool,
	},
}

#[cfg(test)]
mod test_gradient {
	use crate::messages::input_mapper::utility_types::input_mouse::EditorMouseState;
	use crate::messages::input_mapper::utility_types::input_mouse::ScrollDelta;
	use crate::messages::portfolio::document::graph_operation::utility_types::TransformIn;
	use crate::messages::portfolio::document::utility_types::misc::GroupFolderType;
	use crate::messages::portfolio::document::utility_types::network_interface::{InputConnector, OutputConnector};
	use crate::messages::tool::common_functionality::graph_modification_utils::get_fill_node_id_with_direct_fill_input;
	use crate::messages::tool::common_functionality::graph_modification_utils::get_upstream_gradient_value_node_id;
	pub use crate::test_utils::test_prelude::*;
	use glam::DAffine2;
	use graph_craft::document::value::TaggedValue;
	use graphene_std::color::SRGBA8;
	use graphene_std::list::List;
	use graphene_std::vector::style::{GradientSpreadMethod, build_transform_with_y_preservation};
	use graphene_std::vector::{GradientStop, GradientStops, fill};
	use graphene_std::{Graphic, NodeInputDecleration};

	use super::gradient_space_transform;

	struct ResolvedGradient {
		stops: GradientStops,
		spread_method: GradientSpreadMethod,
		transform: DAffine2,
	}

	impl ResolvedGradient {
		fn new(stops: GradientStops, appearance: super::GradientAppearance) -> Self {
			Self {
				stops,
				spread_method: appearance.spread_method,
				transform: appearance.transform,
			}
		}

		fn start(&self) -> DVec2 {
			self.transform.transform_point2(DVec2::ZERO)
		}

		fn end(&self) -> DVec2 {
			self.transform.transform_point2(DVec2::X)
		}
	}

	fn transform_from_line(start: DVec2, end: DVec2) -> DAffine2 {
		build_transform_with_y_preservation(DAffine2::IDENTITY, start, end)
	}

	async fn get_gradients_from_fill(editor: &mut EditorTestUtils) -> Vec<(ResolvedGradient, DAffine2)> {
		let document = editor.active_document();
		document
			.metadata()
			.all_layers()
			.filter_map(|layer| {
				// Only read Fill-owned gradient values, not chains
				let fill_node_id = get_fill_node_id_with_direct_fill_input(layer, &document.network_interface)?;
				let fill_node = document.network_interface.document_network().nodes.get(&fill_node_id)?;

				let stops = match fill_node.inputs.get(fill::FillInput::<List<Graphic>>::INDEX)?.as_value()? {
					TaggedValue::Gradient(stops) => stops.clone(),
					_ => return None,
				};

				let spread_method = match fill_node.inputs.get(fill::SpreadMethodInput::INDEX).and_then(|input| input.as_value()) {
					Some(&TaggedValue::GradientSpreadMethod(value)) => value,
					_ => GradientSpreadMethod::default(),
				};

				let local_transform = match fill_node.inputs.get(fill::TransformInput::INDEX).and_then(|input| input.as_value()) {
					Some(&TaggedValue::OptionalDAffine2(Some(value))) => value,
					_ => DAffine2::IDENTITY,
				};

				let gradient = ResolvedGradient {
					stops,
					spread_method,
					transform: local_transform,
				};

				let transform = gradient_space_transform(layer, document);
				Some((gradient, transform))
			})
			.collect()
	}

	async fn get_gradients_from_chain(editor: &mut EditorTestUtils) -> Vec<(ResolvedGradient, DAffine2)> {
		let document = editor.active_document();
		document
			.metadata()
			.all_layers()
			.filter_map(|layer| {
				// Only read actual gradient chains, not Fill-owned gradient values
				get_upstream_gradient_value_node_id(layer, &document.network_interface)?;

				let (gradient, appearance, _) = super::resolve_gradient(layer, &document.network_interface)?;
				let gradient = ResolvedGradient::new(gradient, appearance);
				let transform = gradient_space_transform(layer, document);
				Some((gradient, transform))
			})
			.collect()
	}

	async fn get_gradient_from_fill(editor: &mut EditorTestUtils) -> (ResolvedGradient, DAffine2) {
		let gradients = get_gradients_from_fill(editor).await;
		assert_eq!(gradients.len(), 1, "Expected 1 gradient fill, found {}", gradients.len());

		gradients.into_iter().next().unwrap()
	}

	async fn get_gradient_from_chain(editor: &mut EditorTestUtils) -> (ResolvedGradient, DAffine2) {
		let gradients = get_gradients_from_chain(editor).await;
		assert_eq!(gradients.len(), 1, "Expected 1 gradient chain, found {}", gradients.len());
		gradients.into_iter().next().unwrap()
	}

	fn assert_stops_at_positions(actual_positions: &[f64], expected_positions: &[f64], tolerance: f64) {
		assert_eq!(
			actual_positions.len(),
			expected_positions.len(),
			"Expected {} stops, found {}",
			expected_positions.len(),
			actual_positions.len()
		);

		for (i, (actual, expected)) in actual_positions.iter().zip(expected_positions.iter()).enumerate() {
			assert!((actual - expected).abs() < tolerance, "Stop {i}: Expected position near {expected}, got {actual}");
		}
	}

	async fn create_gradient_list_layer(editor: &mut EditorTestUtils) -> LayerNodeIdentifier {
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();

		let gradient_node_id = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_value::IDENTIFIER)).await;

		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(gradient_node_id, 0),
				input_connector: InputConnector::node(layer.to_node(), 1),
			})
			.await;

		editor
			.handle_message(NodeGraphMessage::SetInputValue {
				node_id: gradient_node_id,
				input_index: 1,
				value: TaggedValue::Gradient(GradientStops::new([
					GradientStop {
						position: 0.,
						midpoint: 0.5,
						color: Color::RED,
					},
					GradientStop {
						position: 1.,
						midpoint: 0.5,
						color: Color::BLUE,
					},
				])),
			})
			.await;

		layer
	}

	async fn create_fill_gradient_chain_layer(editor: &mut EditorTestUtils) -> LayerNodeIdentifier {
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let document = editor.active_document();
		let layer = document.metadata().all_layers().next().unwrap();
		let fill_node_id = get_fill_node_id_with_direct_fill_input(layer, &document.network_interface).expect("Fill node should exist");

		let gradient_node_id = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_value::IDENTIFIER)).await;

		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(gradient_node_id, 0),
				input_connector: InputConnector::node(fill_node_id, fill::FillInput::<List<Graphic>>::INDEX),
			})
			.await;

		editor
			.handle_message(NodeGraphMessage::SetInputValue {
				node_id: gradient_node_id,
				input_index: 1,
				value: TaggedValue::Gradient(GradientStops::new([
					GradientStop {
						position: 0.,
						midpoint: 0.5,
						color: Color::RED,
					},
					GradientStop {
						position: 1.,
						midpoint: 0.5,
						color: Color::BLUE,
					},
				])),
			})
			.await;

		layer
	}

	#[tokio::test]
	async fn ignore_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Gradient, 2., 2., 4., 4., ModifierKeys::empty()).await;
		assert!(get_gradients_from_fill(&mut editor).await.is_empty());
		assert!(get_gradients_from_chain(&mut editor).await.is_empty());
	}

	#[tokio::test]
	async fn ignore_raster() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.create_raster_image(Image::new(100, 100, Color::WHITE), Some((0., 0.))).await;
		editor.drag_tool(ToolType::Gradient, 2., 2., 4., 4., ModifierKeys::empty()).await;
		assert!(get_gradients_from_fill(&mut editor).await.is_empty());
		assert!(get_gradients_from_chain(&mut editor).await.is_empty());
	}

	#[tokio::test]
	async fn simple_draw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 2., 3., 24., 4., ModifierKeys::empty()).await;

		let (gradient, transform) = get_gradient_from_fill(&mut editor).await;

		// Gradient goes from primary color to secondary color
		let stops = gradient.stops.iter().map(|stop| (stop.position, SRGBA8::from(stop.color))).collect::<Vec<_>>();
		assert_eq!(stops, vec![(0., SRGBA8::from(Color::GREEN)), (1., SRGBA8::from(Color::BLUE))]);
		assert!(transform.transform_point2(gradient.start()).abs_diff_eq(DVec2::new(2., 3.), 1e-10));
		assert!(transform.transform_point2(gradient.end()).abs_diff_eq(DVec2::new(24., 4.), 1e-10));
	}

	#[tokio::test]
	async fn draw_updates_fill_gradient_chain_line() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = create_fill_gradient_chain_layer(&mut editor).await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;
		editor.drag_tool(ToolType::Gradient, 2., 3., 24., 4., ModifierKeys::empty()).await;

		let (gradient, transform) = get_gradient_from_chain(&mut editor).await;

		// Gradient line is updated while existing stops are preserved
		assert!(transform.transform_point2(gradient.start()).abs_diff_eq(DVec2::new(2., 3.), 1e-10));
		assert!(transform.transform_point2(gradient.end()).abs_diff_eq(DVec2::new(24., 4.), 1e-10));
	}

	#[tokio::test]
	async fn snap_simple_draw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				angle_radians: f64::consts::FRAC_PI_8,
			})
			.await;
		let start = DVec2::new(0., 0.);
		let end = DVec2::new(24., 4.);
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Gradient, start.x, start.y, end.x, end.y, ModifierKeys::SHIFT).await;

		let (gradient, transform) = get_gradient_from_fill(&mut editor).await;

		assert!(transform.transform_point2(gradient.start()).abs_diff_eq(start, 1e-10));

		// 15 degrees from horizontal
		let angle = f64::to_radians(15.);
		let direction = DVec2::new(angle.cos(), angle.sin());
		let expected = start + direction * (end - start).length();
		assert!(transform.transform_point2(gradient.end()).abs_diff_eq(expected, 1e-10));
	}

	#[tokio::test]
	async fn transformed_draw() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor
			.handle_message(NavigationMessage::CanvasTiltSet {
				angle_radians: f64::consts::FRAC_PI_8,
			})
			.await;
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;

		// Group rectangle
		let group_folder_type = GroupFolderType::Layer;
		editor.handle_message(DocumentMessage::GroupSelectedLayers { group_folder_type }).await;
		let metadata = editor.active_document().metadata();
		let mut layers = metadata.all_layers();
		let folder = layers.next().unwrap();
		let rectangle = layers.next().unwrap();
		assert_eq!(rectangle.parent(metadata), Some(folder));

		// Transform the group
		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer: folder,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1., 2.), 0., -DVec2::X * 10.),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		editor.drag_tool(ToolType::Gradient, 2., 3., 24., 4., ModifierKeys::empty()).await;

		let (gradient, transform) = get_gradient_from_fill(&mut editor).await;

		assert!(transform.transform_point2(gradient.start()).abs_diff_eq(DVec2::new(2., 3.), 1e-10));
		assert!(transform.transform_point2(gradient.end()).abs_diff_eq(DVec2::new(24., 4.), 1e-10));
	}

	#[tokio::test]
	async fn click_to_insert_stop() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		// Get initial gradient state (should have 2 stops)
		let (initial_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 2, "Expected 2 stops, found {}", initial_gradient.stops.len());

		editor.select_tool(ToolType::Gradient).await;
		editor.move_mouse(25., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(25., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(25., 0., ModifierKeys::empty()).await;

		// Check that a new stop has been added
		let (updated_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 3, "Expected 3 stops, found {}", updated_gradient.stops.len());

		let positions: Vec<f64> = updated_gradient.stops.iter().map(|stop| stop.position).collect();
		assert!(
			positions.iter().any(|pos| (pos - 0.25).abs() < 0.1),
			"Expected to find a stop near position 0.25, but found: {positions:?}"
		);
	}

	#[tokio::test]
	async fn dragging_endpoint_sets_correct_point() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.handle_message(NavigationMessage::CanvasZoomSet { zoom_factor: 2. }).await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;

		let document = editor.active_document();
		let selected_layer = document.network_interface.selected_nodes().selected_layers(document.metadata()).next().unwrap();
		editor
			.handle_message(GraphOperationMessage::TransformSet {
				layer: selected_layer,
				transform: DAffine2::from_scale_angle_translation(DVec2::new(1.5, 0.8), 0.3, DVec2::new(10., -5.)),
				transform_in: TransformIn::Local,
				skip_rerender: false,
			})
			.await;

		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;

		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		// Get the initial gradient state
		let (initial_gradient, transform) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 2, "Expected 2 stops, found {}", initial_gradient.stops.len());

		// Verify initial gradient endpoints in viewport space
		let initial_start = transform.transform_point2(initial_gradient.start());
		let initial_end = transform.transform_point2(initial_gradient.end());
		assert!(initial_start.abs_diff_eq(DVec2::new(0., 0.), 1e-10));
		assert!(initial_end.abs_diff_eq(DVec2::new(100., 0.), 1e-10));

		editor.select_tool(ToolType::Gradient).await;

		// Simulate dragging the end point to a new position (100, 50)
		let start_pos = DVec2::new(100., 0.);
		let end_pos = DVec2::new(100., 50.);

		editor.move_mouse(start_pos.x, start_pos.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(start_pos.x, start_pos.y, ModifierKeys::empty()).await;
		editor.move_mouse(end_pos.x, end_pos.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		editor
			.mouseup(
				EditorMouseState {
					editor_position: end_pos,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Check the updated gradient
		let (updated_gradient, transform) = get_gradient_from_fill(&mut editor).await;

		// Verify the start point hasn't changed
		let updated_start = transform.transform_point2(updated_gradient.start());
		assert!(updated_start.abs_diff_eq(DVec2::new(0., 0.), 1e-10));

		// Verify the end point has been updated to the new position
		let updated_end = transform.transform_point2(updated_gradient.end());
		assert!(updated_end.abs_diff_eq(DVec2::new(100., 50.), 1e-10), "Expected end point at (100, 50), got {updated_end:?}");
	}

	#[tokio::test]
	async fn dragging_stop_reorders_gradient() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		editor.select_tool(ToolType::Gradient).await;

		// Add a middle stop at 25%
		editor.move_mouse(25., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(25., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(25., 0., ModifierKeys::empty()).await;

		let (initial_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 3, "Expected 3 stops, found {}", initial_gradient.stops.len());

		// Verify initial stop positions and colors
		let mut stops = initial_gradient.stops.clone();
		stops.sort();

		let positions: Vec<f64> = stops.iter().map(|stop| stop.position).collect();
		assert_stops_at_positions(&positions, &[0., 0.25, 1.], 0.1);

		let middle_color = SRGBA8::from(stops.color[1]);

		// Simulate dragging the middle stop to position 0.8
		let click_position = DVec2::new(25., 0.);
		editor
			.mousedown(
				EditorMouseState {
					editor_position: click_position,
					mouse_keys: MouseKeys::LEFT,
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let drag_position = DVec2::new(80., 0.);
		editor.move_mouse(drag_position.x, drag_position.y, ModifierKeys::empty(), MouseKeys::LEFT).await;

		editor
			.mouseup(
				EditorMouseState {
					editor_position: drag_position,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		let (updated_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 3, "Expected 3 stops after dragging, found {}", updated_gradient.stops.len());

		// Verify updated stop positions and colors
		let mut updated_stops = updated_gradient.stops.clone();
		updated_stops.sort();

		// Check positions are now correctly ordered
		let updated_positions: Vec<f64> = updated_stops.iter().map(|stop| stop.position).collect();
		assert_stops_at_positions(&updated_positions, &[0., 0.8, 1.], 0.1);

		// Colors should maintain their associations with the stop points
		assert_eq!(SRGBA8::from(updated_stops.color[0]), SRGBA8::from(Color::GREEN));
		assert_eq!(SRGBA8::from(updated_stops.color[1]), middle_color);
		assert_eq!(SRGBA8::from(updated_stops.color[2]), SRGBA8::from(Color::BLUE));
	}

	#[tokio::test]
	async fn select_and_delete_removes_stop() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.select_secondary_color(Color::BLUE).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;

		// Get initial gradient state (should have 2 stops)
		let (initial_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(initial_gradient.stops.len(), 2, "Expected 2 stops, found {}", initial_gradient.stops.len());

		editor.select_tool(ToolType::Gradient).await;

		// Add two middle stops
		editor.move_mouse(25., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(25., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(25., 0., ModifierKeys::empty()).await;

		editor.move_mouse(75., 0., ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(75., 0., ModifierKeys::empty()).await;
		editor.left_mouseup(75., 0., ModifierKeys::empty()).await;

		let (updated_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(updated_gradient.stops.len(), 4, "Expected 4 stops, found {}", updated_gradient.stops.len());

		let positions: Vec<f64> = updated_gradient.stops.iter().map(|stop| stop.position).collect();

		// Use helper function to verify positions
		assert_stops_at_positions(&positions, &[0., 0.25, 0.75, 1.], 0.05);

		// Select the stop at position 0.75 and delete it
		let position2 = DVec2::new(75., 0.);
		editor.move_mouse(position2.x, position2.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(position2.x, position2.y, ModifierKeys::empty()).await;
		editor
			.mouseup(
				EditorMouseState {
					editor_position: position2,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		editor.press(Key::Delete, ModifierKeys::empty()).await;

		// Verify we now have 3 stops
		let (final_gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(final_gradient.stops.len(), 3, "Expected 3 stops after deletion, found {}", final_gradient.stops.len());

		let final_positions: Vec<f64> = final_gradient.stops.iter().map(|stop| stop.position).collect();

		// Verify final positions with helper function
		assert_stops_at_positions(&final_positions, &[0., 0.25, 1.], 0.05);

		// Additional verification that 0.75 stop is gone
		assert!(!final_positions.iter().any(|pos| (pos - 0.75).abs() < 0.05), "Stop at position 0.75 should have been deleted");
	}

	#[tokio::test]
	async fn delete_removes_layer_when_no_stop_selected() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Create a layer and switch to the Gradient tool without engaging any gradient handle
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.select_tool(ToolType::Gradient).await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 1, "Expected the rectangle layer to exist");

		// With no color stop selected, Delete should fall through to deleting the selected layer
		editor.press(Key::Delete, ModifierKeys::empty()).await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 0, "Expected the layer to be deleted");
	}

	#[tokio::test]
	async fn delete_removes_layer_after_drawing_gradient() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;

		// Draw a fresh gradient, which leaves `selected_gradient` set to the `New` drag target rather than a selected stop
		editor.drag_tool(ToolType::Rectangle, -5., -3., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Gradient, 0., 0., 100., 0., ModifierKeys::empty()).await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 1, "Expected the rectangle layer to exist");

		// Since no stop is selected (`New` isn't a deletable handle), Delete should still delete the layer
		editor.press(Key::Delete, ModifierKeys::empty()).await;
		assert_eq!(editor.active_document().metadata().all_layers().count(), 0, "Expected the layer to be deleted after drawing a gradient");
	}

	#[tokio::test]
	async fn change_spread_method() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.drag_tool(ToolType::Gradient, 10., 10., 90., 90., ModifierKeys::empty()).await;

		// Verify default spread method is Pad
		let (gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(gradient.spread_method, GradientSpreadMethod::Pad);

		// Update spread method to Repeat
		editor
			.handle_message(GradientToolMessage::UpdateOptions {
				options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Repeat),
			})
			.await;

		let (gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(gradient.spread_method, GradientSpreadMethod::Repeat);

		// Update spread method to Reflect
		editor
			.handle_message(GradientToolMessage::UpdateOptions {
				options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Reflect),
			})
			.await;

		let (gradient, _) = get_gradient_from_fill(&mut editor).await;
		assert_eq!(gradient.spread_method, GradientSpreadMethod::Reflect);
	}

	#[tokio::test]
	async fn change_spread_method_chain() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = create_fill_gradient_chain_layer(&mut editor).await;
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;
		editor.select_tool(ToolType::Gradient).await;

		// Verify default spread method is Pad
		let (gradient, _) = get_gradient_from_chain(&mut editor).await;
		assert_eq!(gradient.spread_method, GradientSpreadMethod::Pad);

		// Update spread method to Repeat
		editor
			.handle_message(GradientToolMessage::UpdateOptions {
				options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Repeat),
			})
			.await;

		let (gradient, _) = get_gradient_from_chain(&mut editor).await;
		assert_eq!(gradient.spread_method, GradientSpreadMethod::Repeat);

		// Update spread method to Reflect
		editor
			.handle_message(GradientToolMessage::UpdateOptions {
				options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Reflect),
			})
			.await;

		let (gradient, _) = get_gradient_from_chain(&mut editor).await;
		assert_eq!(gradient.spread_method, GradientSpreadMethod::Reflect);
	}

	#[tokio::test]
	async fn gradient_list_layer_drag_endpoint() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = create_gradient_list_layer(&mut editor).await;

		// Create original transform for the control geometry and apply it
		let initial_start = DVec2::new(10., 50.);
		let initial_end = DVec2::new(200., 50.);
		let stops = GradientStops::new([
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: Color::RED,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: Color::BLUE,
			},
		]);
		editor.handle_message(GraphOperationMessage::GradientStopsSet { layer, stops }).await;
		editor
			.handle_message(GraphOperationMessage::GradientTransformSet {
				layer,
				transform: transform_from_line(initial_start, initial_end),
			})
			.await;

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;

		let document = editor.active_document();
		let space_transform = gradient_space_transform(layer, document);
		let (gradient, appearance, _) = super::resolve_gradient(layer, &document.network_interface).unwrap();
		let gradient = ResolvedGradient::new(gradient, appearance);
		let viewport_start = space_transform.transform_point2(gradient.start());
		let viewport_end = space_transform.transform_point2(gradient.end());

		// Drag target of the end point, move 80px down
		let new_viewport_end = viewport_end + DVec2::new(0., 80.);
		editor.select_tool(ToolType::Gradient).await;
		editor.move_mouse(viewport_end.x, viewport_end.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(viewport_end.x, viewport_end.y, ModifierKeys::empty()).await;
		editor.move_mouse(new_viewport_end.x, new_viewport_end.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		editor
			.mouseup(
				EditorMouseState {
					editor_position: new_viewport_end,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Verify if the gradient position is updated correctly
		let document = editor.active_document();
		let (updated, appearance, _) = super::resolve_gradient(layer, &document.network_interface).expect("Gradient should exist after drag");
		let updated = ResolvedGradient::new(updated, appearance);
		let updated_space_transform = gradient_space_transform(layer, document);
		let updated_viewport_start = updated_space_transform.transform_point2(updated.start());
		let updated_viewport_end = updated_space_transform.transform_point2(updated.end());

		assert!(
			updated_viewport_start.abs_diff_eq(viewport_start, 1.),
			"Start should not move. Expected {viewport_start:?}, got {updated_viewport_start:?}"
		);
		assert!(
			updated_viewport_end.abs_diff_eq(new_viewport_end, 1.),
			"End should move to new position. Expected {new_viewport_end:?}, got {updated_viewport_end:?}"
		);
	}

	#[tokio::test]
	async fn gradient_list_layer_preserves_stops() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		let layer = create_gradient_list_layer(&mut editor).await;

		// Set up a 3-stop gradient with distinct colors
		let original_stops = GradientStops::new([
			GradientStop {
				position: 0.,
				midpoint: 0.5,
				color: Color::RED,
			},
			GradientStop {
				position: 0.5,
				midpoint: 0.5,
				color: Color::GREEN,
			},
			GradientStop {
				position: 1.,
				midpoint: 0.5,
				color: Color::BLUE,
			},
		]);
		let initial_start = DVec2::new(10., 50.);
		let initial_end = DVec2::new(200., 50.);
		editor.handle_message(GraphOperationMessage::GradientStopsSet { layer, stops: original_stops.clone() }).await;
		editor
			.handle_message(GraphOperationMessage::GradientTransformSet {
				layer,
				transform: transform_from_line(initial_start, initial_end),
			})
			.await;

		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;

		let document = editor.active_document();
		let space_transform = gradient_space_transform(layer, document);
		let (gradient, appearance, _) = super::resolve_gradient(layer, &document.network_interface).unwrap();
		let gradient = ResolvedGradient::new(gradient, appearance);
		let viewport_end = space_transform.transform_point2(gradient.end());

		// Drag the end point 80px down
		let new_viewport_end = viewport_end + DVec2::new(0., 80.);
		editor.select_tool(ToolType::Gradient).await;
		editor.move_mouse(viewport_end.x, viewport_end.y, ModifierKeys::empty(), MouseKeys::empty()).await;
		editor.left_mousedown(viewport_end.x, viewport_end.y, ModifierKeys::empty()).await;
		editor.move_mouse(new_viewport_end.x, new_viewport_end.y, ModifierKeys::empty(), MouseKeys::LEFT).await;
		editor
			.mouseup(
				EditorMouseState {
					editor_position: new_viewport_end,
					mouse_keys: MouseKeys::empty(),
					scroll_delta: ScrollDelta::default(),
				},
				ModifierKeys::empty(),
			)
			.await;

		// Verify stops are preserved after dragging
		let document = editor.active_document();
		let (updated, appearance, _) = super::resolve_gradient(layer, &document.network_interface).expect("Gradient should exist after drag");
		let updated = ResolvedGradient::new(updated, appearance);

		assert_eq!(updated.stops.len(), 3, "Stop count should be preserved");
		assert_stops_at_positions(&updated.stops.position, &[0., 0.5, 1.], 1e-10);
		assert_eq!(SRGBA8::from(updated.stops.color[0]), SRGBA8::from(Color::RED), "First stop color should be preserved");
		assert_eq!(SRGBA8::from(updated.stops.color[1]), SRGBA8::from(Color::GREEN), "Middle stop color should be preserved");
		assert_eq!(SRGBA8::from(updated.stops.color[2]), SRGBA8::from(Color::BLUE), "Last stop color should be preserved");
	}

	// When the gradient chain feeds a 'Fill' node's secondary input it's an unencapsulated side-branch (no layer
	// background to lay it out), so a node inserted there must be placed onto the displaced feeder's spot in absolute
	// graph space rather than stranded at the origin.
	#[tokio::test]
	async fn gradient_chain_node_on_fill_secondary_input_takes_feeder_slot() {
		use graphene_std::vector::style::GradientSpreadMethod;

		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Ellipse, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let layer = editor.active_document().metadata().all_layers().next().unwrap();

		// Find the 'Fill' node in the layer's primary chain.
		let fill_reference = DefinitionIdentifier::ProtoNode(graphene_std::vector::fill::IDENTIFIER);
		let fill_node_id = {
			let network_interface = &editor.active_document().network_interface;
			network_interface
				.document_network()
				.nodes
				.keys()
				.copied()
				.find(|node_id| network_interface.reference(node_id, &[]).as_ref() == Some(&fill_reference))
				.expect("Fill node should exist")
		};

		// Feed a 'Gradient Value' node into the Fill node's secondary (fill) input.
		let gradient_value_id = editor.create_node_by_name(DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::gradient_value::IDENTIFIER)).await;
		editor
			.handle_message(NodeGraphMessage::CreateWire {
				output_connector: OutputConnector::node(gradient_value_id, 0),
				input_connector: InputConnector::node(fill_node_id, 1),
			})
			.await;
		editor
			.handle_message(NodeGraphMessage::SetInputValue {
				node_id: gradient_value_id,
				input_index: 1,
				value: TaggedValue::Gradient(GradientStops::new([
					GradientStop {
						position: 0.,
						midpoint: 0.5,
						color: Color::RED,
					},
					GradientStop {
						position: 1.,
						midpoint: 0.5,
						color: Color::BLUE,
					},
				])),
			})
			.await;

		// Move the feeder off the origin so its slot is unambiguous, then record where it sits.
		editor
			.handle_message(NodeGraphMessage::ShiftNodePosition {
				node_id: gradient_value_id,
				x: 4,
				y: 6,
			})
			.await;
		let feeder_position = editor.active_document_mut().network_interface.position(&gradient_value_id, &[]).expect("Gradient Value position");

		// Set the spread method through the tool, which splices a 'Spread Method' node onto the Fill's fill input wire.
		editor.handle_message(NodeGraphMessage::SelectedNodesSet { nodes: vec![layer.to_node()] }).await;
		editor.select_tool(ToolType::Gradient).await;
		editor
			.handle_message(GradientToolMessage::UpdateOptions {
				options: GradientOptionsUpdate::SetSpreadMethod(GradientSpreadMethod::Reflect),
			})
			.await;

		let spread_reference = DefinitionIdentifier::ProtoNode(graphene_std::math_nodes::spread_method::IDENTIFIER);
		let spread_node_id = {
			let network_interface = &editor.active_document().network_interface;
			network_interface
				.document_network()
				.nodes
				.keys()
				.copied()
				.find(|node_id| network_interface.reference(node_id, &[]).as_ref() == Some(&spread_reference))
				.expect("Spread Method node should have been inserted")
		};

		let spread_position = editor.active_document_mut().network_interface.position(&spread_node_id, &[]).expect("Spread Method position");
		let feeder_position_after = editor.active_document_mut().network_interface.position(&gradient_value_id, &[]).expect("Gradient Value position after");

		assert_eq!(spread_position, feeder_position, "the inserted node should occupy the feeder's former slot, not the graph origin");
		assert_eq!(
			feeder_position_after,
			feeder_position - glam::IVec2::new(crate::consts::NODE_CHAIN_WIDTH, 0),
			"the feeder's branch should shift one chain-width left to make room"
		);
	}
}
