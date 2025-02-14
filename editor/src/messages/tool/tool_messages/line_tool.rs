use super::tool_prelude::*;
use crate::consts::{BOUNDS_SELECT_THRESHOLD, DEFAULT_STROKE_WIDTH, LINE_ROTATE_SNAP_ANGLE};
use crate::messages::portfolio::document::node_graph::document_node_definitions::resolve_document_node_type;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::{document_metadata::LayerNodeIdentifier, network_interface::InputConnector};
use crate::messages::tool::common_functionality::auto_panning::AutoPanning;
use crate::messages::tool::common_functionality::color_selector::{ToolColorOptions, ToolColorType};
use crate::messages::tool::common_functionality::graph_modification_utils::{self, NodeGraphLayer};
use crate::messages::tool::common_functionality::snapping::{SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnapTypeConfiguration};

use graph_craft::document::{value::TaggedValue, NodeId, NodeInput};
use graphene_core::Color;

#[derive(Default)]
pub struct LineTool {
	fsm_state: LineToolFsmState,
	tool_data: LineToolData,
	options: LineOptions,
}

pub struct LineOptions {
	line_weight: f64,
	stroke: ToolColorOptions,
}

impl Default for LineOptions {
	fn default() -> Self {
		Self {
			line_weight: DEFAULT_STROKE_WIDTH,
			stroke: ToolColorOptions::new_primary(),
		}
	}
}

#[impl_message(Message, ToolMessage, Line)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum LineToolMessage {
	// Standard messages
	Overlays(OverlayContext),
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	DragStart,
	DragStop,
	PointerMove { center: Key, lock_angle: Key, snap_angle: Key },
	PointerOutsideViewport { center: Key, lock_angle: Key, snap_angle: Key },
	UpdateOptions(LineOptionsUpdate),
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum LineOptionsUpdate {
	LineWeight(f64),
	StrokeColor(Option<Color>),
	StrokeColorType(ToolColorType),
	WorkingColors(Option<Color>, Option<Color>),
}

impl ToolMetadata for LineTool {
	fn icon_name(&self) -> String {
		"VectorLineTool".into()
	}
	fn tooltip(&self) -> String {
		"Line Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Line
	}
}

fn create_weight_widget(line_weight: f64) -> WidgetHolder {
	NumberInput::new(Some(line_weight))
		.unit(" px")
		.label("Weight")
		.min(0.)
		.max((1_u64 << f64::MANTISSA_DIGITS) as f64)
		.on_update(|number_input: &NumberInput| LineToolMessage::UpdateOptions(LineOptionsUpdate::LineWeight(number_input.value.unwrap())).into())
		.widget_holder()
}

impl LayoutHolder for LineTool {
	fn layout(&self) -> Layout {
		let mut widgets = self.options.stroke.create_widgets(
			"Stroke",
			true,
			|_| LineToolMessage::UpdateOptions(LineOptionsUpdate::StrokeColor(None)).into(),
			|color_type: ToolColorType| WidgetCallback::new(move |_| LineToolMessage::UpdateOptions(LineOptionsUpdate::StrokeColorType(color_type.clone())).into()),
			|color: &ColorInput| LineToolMessage::UpdateOptions(LineOptionsUpdate::StrokeColor(color.value.as_solid())).into(),
		);
		widgets.push(Separator::new(SeparatorType::Unrelated).widget_holder());
		widgets.push(create_weight_widget(self.options.line_weight));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for LineTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let ToolMessage::Line(LineToolMessage::UpdateOptions(action)) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, tool_data, &self.options, responses, true);
			return;
		};
		match action {
			LineOptionsUpdate::LineWeight(line_weight) => self.options.line_weight = line_weight,
			LineOptionsUpdate::StrokeColor(color) => {
				self.options.stroke.custom_color = color;
				self.options.stroke.color_type = ToolColorType::Custom;
			}
			LineOptionsUpdate::StrokeColorType(color_type) => self.options.stroke.color_type = color_type,
			LineOptionsUpdate::WorkingColors(primary, secondary) => {
				self.options.stroke.primary_working_color = primary;
				self.options.stroke.secondary_working_color = secondary;
			}
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			LineToolFsmState::Ready => actions!(LineToolMessageDiscriminant; DragStart, PointerMove),
			LineToolFsmState::Drawing => actions!(LineToolMessageDiscriminant; DragStop, PointerMove, Abort),
		}
	}
}

impl ToolTransition for LineTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|overlay_context| LineToolMessage::Overlays(overlay_context).into()),
			tool_abort: Some(LineToolMessage::Abort.into()),
			working_color_changed: Some(LineToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum LineToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(Clone, Debug, Default)]
struct LineToolData {
	drag_start: DVec2,
	drag_current: DVec2,
	angle: f64,
	weight: f64,
	selected_layers_with_position: HashMap<LayerNodeIdentifier, [DVec2; 2]>,
	editing_layer: Option<LayerNodeIdentifier>,
	snap_manager: SnapManager,
	auto_panning: AutoPanning,
	start_click: bool,
	end_click: bool,
}

impl Fsm for LineToolFsmState {
	type ToolData = LineToolData;
	type ToolOptions = LineOptions;

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, tool_action_data: &mut ToolActionHandlerData, tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = tool_action_data;

		let ToolMessage::Line(event) = event else { return self };
		match (self, event) {
			(_, LineToolMessage::Overlays(mut overlay_context)) => {
				tool_data.snap_manager.draw_overlays(SnapData::new(document, input), &mut overlay_context);

				tool_data.selected_layers_with_position = document
					.network_interface
					.selected_nodes(&[])
					.unwrap()
					.selected_visible_and_unlocked_layers(&document.network_interface)
					.filter_map(|layer| {
						let graph_layer = NodeGraphLayer::new(layer, &document.network_interface);
						let node_inputs = graph_layer.find_node_inputs("Line")?;
						let transform_inputs = graph_layer.find_node_inputs("Transform");

						let (Some(&TaggedValue::DVec2(start)), Some(&TaggedValue::DVec2(end))) = (node_inputs[1].as_value(), node_inputs[2].as_value()) else {
							return None;
						};

						let mut document_transform = DAffine2::IDENTITY;
						if let Some(transform_inputs) = transform_inputs {
							let (Some(&TaggedValue::DVec2(translation)), Some(&TaggedValue::F64(angle)), Some(&TaggedValue::DVec2(scale))) =
								(transform_inputs[1].as_value(), transform_inputs[2].as_value(), transform_inputs[3].as_value())
							else {
								return None;
							};

							document_transform = DAffine2::from_scale_angle_translation(scale, angle, translation);
						}

						let viewport_transform = document.metadata().document_to_viewport;
						let transform = document_transform * viewport_transform;
						if (start.x - end.x).abs() > f64::EPSILON * 1000. && (start.y - end.y).abs() > f64::EPSILON * 1000. {
							overlay_context.square(transform.transform_point2(start), Some(6.), None, None);
							overlay_context.square(transform.transform_point2(end), Some(6.), None, None);
						}

						Some((layer, [start, end]))
					})
					.collect::<HashMap<LayerNodeIdentifier, [DVec2; 2]>>();

				self
			}
			(LineToolFsmState::Ready, LineToolMessage::DragStart) => {
				let point = SnapCandidatePoint::handle(document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position));
				let snapped = tool_data.snap_manager.free_snap(&SnapData::new(document, input), &point, SnapTypeConfiguration::default());
				tool_data.drag_start = snapped.snapped_point_document;

				for (layer, [start, end]) in tool_data.selected_layers_with_position.iter() {
					let transform = document.metadata().transform_to_viewport(*layer);
					let viewport_x = transform.transform_vector2(DVec2::X).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
					let viewport_y = transform.transform_vector2(DVec2::Y).normalize_or_zero() * BOUNDS_SELECT_THRESHOLD;
					let threshold_x = transform.inverse().transform_vector2(viewport_x).length();
					let threshold_y = transform.inverse().transform_vector2(viewport_y).length();

					let graph_layer = NodeGraphLayer::new(*layer, &document.network_interface);
					let transform_inputs = graph_layer.find_node_inputs("Transform");

					let mut transform = DAffine2::IDENTITY;
					if let Some(transform_inputs) = transform_inputs {
						if let (Some(&TaggedValue::DVec2(translation)), Some(&TaggedValue::F64(angle)), Some(&TaggedValue::DVec2(scale))) =
							(transform_inputs[1].as_value(), transform_inputs[2].as_value(), transform_inputs[3].as_value())
						{
							transform = DAffine2::from_scale_angle_translation(scale, angle, translation);
						};
					}

					let [start, end] = [start, end].map(|point| transform.transform_point2(*point));
					tool_data.start_click = (tool_data.drag_start.y - start.y).abs() < threshold_y && (tool_data.drag_start.x - start.x).abs() < threshold_x;
					tool_data.end_click = (tool_data.drag_start.y - end.y).abs() < threshold_y && (tool_data.drag_start.x - end.x).abs() < threshold_x;

					if tool_data.start_click || tool_data.end_click {
						tool_data.drag_start = if tool_data.start_click { end } else { start };
						tool_data.editing_layer = Some(*layer);
						return LineToolFsmState::Drawing;
					}
				}

				responses.add(DocumentMessage::StartTransaction);

				let node_type = resolve_document_node_type("Line").expect("Line node does not exist");
				let node = node_type.node_template_input_override([
					None,
					Some(NodeInput::value(
						TaggedValue::DVec2(document.metadata().document_to_viewport.transform_point2(tool_data.drag_start)),
						false,
					)),
					Some(NodeInput::value(
						TaggedValue::DVec2(document.metadata().document_to_viewport.transform_point2(tool_data.drag_start)),
						false,
					)),
				]);
				let nodes = vec![(NodeId(0), node)];

				let layer = graph_modification_utils::new_custom(NodeId::new(), nodes, document.new_layer_bounding_artboard(input), responses);
				responses.add(Message::StartBuffer);

				tool_options.stroke.apply_stroke(tool_options.line_weight, layer, responses);

				tool_data.editing_layer = Some(layer);
				tool_data.angle = 0.;
				tool_data.weight = tool_options.line_weight;

				LineToolFsmState::Drawing
			}
			(LineToolFsmState::Drawing, LineToolMessage::PointerMove { center, snap_angle, lock_angle }) => {
				if tool_data.editing_layer.is_none() {
					return LineToolFsmState::Ready;
				}

				tool_data.drag_current = document.metadata().document_to_viewport.inverse().transform_point2(input.mouse.position); // tool_data.snap_manager.snap_position(responses, document, input.mouse.position);

				let keyboard = &input.keyboard;
				let ignore = vec![tool_data.editing_layer.unwrap()];
				let snap_data = SnapData::ignore(document, input, &ignore);
				let mut document_points = generate_line(tool_data, snap_data, keyboard.key(lock_angle), keyboard.key(snap_angle), keyboard.key(center));

				if tool_data.start_click {
					document_points.swap(0, 1);
				}

				let Some(node_id) = graph_modification_utils::get_line_id(tool_data.editing_layer.unwrap(), &document.network_interface) else {
					return LineToolFsmState::Ready;
				};

				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, 1),
					input: NodeInput::value(TaggedValue::DVec2(document_points[0]), false),
				});
				responses.add(NodeGraphMessage::SetInput {
					input_connector: InputConnector::node(node_id, 2),
					input: NodeInput::value(TaggedValue::DVec2(document_points[1]), false),
				});
				responses.add(NodeGraphMessage::RunDocumentGraph);

				// Auto-panning
				let messages = [
					LineToolMessage::PointerOutsideViewport { center, snap_angle, lock_angle }.into(),
					LineToolMessage::PointerMove { center, snap_angle, lock_angle }.into(),
				];
				tool_data.auto_panning.setup_by_mouse_position(input, &messages, responses);

				self
			}
			(_, LineToolMessage::PointerMove { .. }) => {
				tool_data.snap_manager.preview_draw(&SnapData::new(document, input), input.mouse.position);
				responses.add(OverlaysMessage::Draw);
				self
			}
			(LineToolFsmState::Drawing, LineToolMessage::PointerOutsideViewport { .. }) => {
				// Auto-panning
				let _ = tool_data.auto_panning.shift_viewport(input, responses);

				self
			}
			(state, LineToolMessage::PointerOutsideViewport { center, lock_angle, snap_angle }) => {
				// Auto-panning
				let messages = [
					LineToolMessage::PointerOutsideViewport { center, lock_angle, snap_angle }.into(),
					LineToolMessage::PointerMove { center, lock_angle, snap_angle }.into(),
				];
				tool_data.auto_panning.stop(&messages, responses);

				state
			}
			(LineToolFsmState::Drawing, LineToolMessage::DragStop) => {
				tool_data.snap_manager.cleanup(responses);
				tool_data.editing_layer.take();
				input.mouse.finish_transaction(tool_data.drag_start, responses);
				LineToolFsmState::Ready
			}
			(LineToolFsmState::Drawing, LineToolMessage::Abort) => {
				tool_data.snap_manager.cleanup(responses);
				tool_data.editing_layer.take();
				if !tool_data.start_click && !tool_data.end_click {
					responses.add(DocumentMessage::AbortTransaction);
				}
				LineToolFsmState::Ready
			}
			(_, LineToolMessage::WorkingColorChanged) => {
				responses.add(LineToolMessage::UpdateOptions(LineOptionsUpdate::WorkingColors(
					Some(global_tool_data.primary_color),
					Some(global_tool_data.secondary_color),
				)));
				self
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			LineToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::LmbDrag, "Draw Line"),
				HintInfo::keys([Key::Shift], "15° Increments").prepend_plus(),
				HintInfo::keys([Key::Alt], "From Center").prepend_plus(),
				HintInfo::keys([Key::Control], "Lock Angle").prepend_plus(),
			])]),
			LineToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![
					HintInfo::keys([Key::Shift], "15° Increments"),
					HintInfo::keys([Key::Alt], "From Center"),
					HintInfo::keys([Key::Control], "Lock Angle"),
				]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Crosshair });
	}
}

fn generate_line(tool_data: &mut LineToolData, snap_data: SnapData, lock_angle: bool, snap_angle: bool, center: bool) -> [DVec2; 2] {
	let mut document_points = [tool_data.drag_start, tool_data.drag_current];

	let mut angle = -(document_points[1] - document_points[0]).angle_to(DVec2::X);
	let mut line_length = (document_points[1] - document_points[0]).length();

	if lock_angle {
		angle = tool_data.angle;
	} else if snap_angle {
		let snap_resolution = LINE_ROTATE_SNAP_ANGLE.to_radians();
		angle = (angle / snap_resolution).round() * snap_resolution;
	}

	tool_data.angle = angle;

	if lock_angle {
		let angle_vec = DVec2::new(angle.cos(), angle.sin());
		line_length = (document_points[1] - document_points[0]).dot(angle_vec);
	}

	document_points[1] = document_points[0] + line_length * DVec2::new(angle.cos(), angle.sin());

	let constrained = snap_angle || lock_angle;
	let snap = &mut tool_data.snap_manager;

	let near_point = SnapCandidatePoint::handle_neighbors(document_points[1], [tool_data.drag_start]);
	let far_point = SnapCandidatePoint::handle_neighbors(2. * document_points[0] - document_points[1], [tool_data.drag_start]);
	let config = SnapTypeConfiguration::default();

	if constrained {
		let constraint = SnapConstraint::Line {
			origin: document_points[0],
			direction: document_points[1] - document_points[0],
		};
		if center {
			let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
			let snapped_far = snap.constrained_snap(&snap_data, &far_point, constraint, config);
			let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
			document_points[1] = document_points[0] * 2. - best.snapped_point_document;
			document_points[0] = best.snapped_point_document;
			snap.update_indicator(best);
		} else {
			let snapped = snap.constrained_snap(&snap_data, &near_point, constraint, config);
			document_points[1] = snapped.snapped_point_document;
			snap.update_indicator(snapped);
		}
	} else if center {
		let snapped = snap.free_snap(&snap_data, &near_point, config);
		let snapped_far = snap.free_snap(&snap_data, &far_point, config);
		let best = if snapped_far.other_snap_better(&snapped) { snapped } else { snapped_far };
		document_points[1] = document_points[0] * 2. - best.snapped_point_document;
		document_points[0] = best.snapped_point_document;
		snap.update_indicator(best);
	} else {
		let snapped = snap.free_snap(&snap_data, &near_point, config);
		document_points[1] = snapped.snapped_point_document;
		snap.update_indicator(snapped);
	}

	document_points
}
