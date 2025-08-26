use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::shapes::shape_utility::extract_circular_repeat_parameters;

#[derive(Default, ExtractField)]
pub struct OperationTool {
	fsm_state: OperationToolFsmState,
	tool_data: OperationToolData,
	options: OperationOptions,
}

pub struct OperationOptions {
	operation_type: OperationType,
}

impl Default for OperationOptions {
	fn default() -> Self {
		Self {
			operation_type: OperationType::CircularRepeat,
		}
	}
}

#[impl_message(Message, ToolMessage, Operation)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OperationToolMessage {
	// Standard messages
	Overlays { context: OverlayContext },
	Abort,
	WorkingColorChanged,

	// Tool-specific messages
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	PointerOutsideViewport,
	Undo,
	UpdateOptions { options: OperationOptionsUpdate },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum OperationToolFsmState {
	#[default]
	Ready,
	Drawing,
}

#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OperationOptionsUpdate {
	OperationType(OperationType),
}

impl ToolMetadata for OperationTool {
	fn icon_name(&self) -> String {
		"GeneralOperationTool".into()
	}
	fn tooltip(&self) -> String {
		"Operation Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Operation
	}
}

fn create_operation_type_option_widget(operation_type: OperationType) -> WidgetHolder {
	let entries = vec![vec![
		MenuListEntry::new("Repeat").label("Repeat").on_commit(move |_| {
			OperationToolMessage::UpdateOptions {
				options: OperationOptionsUpdate::OperationType(OperationType::Repeat),
			}
			.into()
		}),
		MenuListEntry::new("Repeat").label("Circular Repeat").on_commit(move |_| {
			OperationToolMessage::UpdateOptions {
				options: OperationOptionsUpdate::OperationType(OperationType::CircularRepeat),
			}
			.into()
		}),
	]];
	DropdownInput::new(entries).selected_index(Some(operation_type as u32)).widget_holder()
}

impl LayoutHolder for OperationTool {
	fn layout(&self) -> Layout {
		let mut widgets = vec![];

		widgets.push(create_operation_type_option_widget(self.options.operation_type));

		Layout::WidgetLayout(WidgetLayout::new(vec![LayoutGroup::Row { widgets }]))
	}
}

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for OperationTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		let ToolMessage::Operation(OperationToolMessage::UpdateOptions { options }) = message else {
			self.fsm_state.process_event(message, &mut self.tool_data, context, &self.options, responses, true);
			return;
		};
		match options {
			OperationOptionsUpdate::OperationType(operation_type) => self.options.operation_type = operation_type,
		}

		self.send_layout(responses, LayoutTarget::ToolOptions);
	}

	fn actions(&self) -> ActionList {
		match self.fsm_state {
			OperationToolFsmState::Ready => actions!(OperationToolMessageDiscriminant;
				Undo,
				DragStart,
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
			OperationToolFsmState::Drawing => actions!(OperationToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Confirm,
				Abort,
			),
		}
	}
}

impl ToolTransition for OperationTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			overlay_provider: Some(|context: OverlayContext| OperationToolMessage::Overlays { context }.into()),
			tool_abort: Some(OperationToolMessage::Abort.into()),
			working_color_changed: Some(OperationToolMessage::WorkingColorChanged.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Debug, Default)]
struct OperationToolData {
	drag_start: DVec2,
	clicked_layer_radius: (LayerNodeIdentifier, f64),
	layers_dragging: Vec<(LayerNodeIdentifier, f64)>,
	initial_center: DVec2,
}

impl OperationToolData {
	fn cleanup(&mut self) {
		self.layers_dragging.clear();
	}
}

impl Fsm for OperationToolFsmState {
	type ToolData = OperationToolData;
	type ToolOptions = OperationOptions;

	fn transition(
		self,
		event: ToolMessage,
		tool_data: &mut Self::ToolData,
		tool_action_data: &mut ToolActionMessageContext,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, .. } = tool_action_data;

		let ToolMessage::Operation(event) = event else { return self };
		match (self, event) {
			(_, OperationToolMessage::Overlays { context: mut overlay_context }) => {
				match self {
					OperationToolFsmState::Ready => {
						for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { continue };
							let viewport = document.metadata().transform_to_viewport(layer);
							let center = viewport.transform_point2(DVec2::ZERO);
							if center.distance(input.mouse.position) < 5. {
								overlay_context.circle(center, 3., None, None);
							}

							overlay_context.outline_vector(&vector, viewport);
						}
						if let Some(layer) = document.click(&input) {
							let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return self };
							let viewport = document.metadata().transform_to_viewport(layer);
							let center = viewport.transform_point2(DVec2::ZERO);
							if center.distance(input.mouse.position) < 5. {
								overlay_context.circle(center, 3., None, None);
							}

							overlay_context.outline_vector(&vector, viewport);
						}
					}
					_ => {
						for layer in tool_data.layers_dragging.iter().map(|(l, _)| l) {
							let Some(vector) = document.network_interface.compute_modified_vector(*layer) else { continue };
							let viewport = document.metadata().transform_to_viewport(*layer);

							overlay_context.outline_vector(&vector, viewport);
						}
					}
				}

				self
			}
			(OperationToolFsmState::Ready, OperationToolMessage::DragStart) => {
				let selected_layers = document
					.network_interface
					.selected_nodes()
					.selected_layers(document.metadata())
					.collect::<HashSet<LayerNodeIdentifier>>();
				let Some(clicked_layer) = document.click(&input) else { return self };
				responses.add(DocumentMessage::StartTransaction);
				let viewport = document.metadata().transform_to_viewport(clicked_layer);
				let center = viewport.transform_point2(DVec2::ZERO);

				if center.distance(input.mouse.position) > 5. {
					return self;
				};

				if selected_layers.contains(&clicked_layer) {
					// store all
					tool_data.layers_dragging = selected_layers
						.iter()
						.map(|layer| {
							let (_angle_offset, radius, _count) = extract_circular_repeat_parameters(Some(*layer), document).unwrap_or((0.0, 0.0, 6));
							if *layer == clicked_layer {
								tool_data.clicked_layer_radius = (*layer, radius)
							}
							(*layer, radius)
						})
						.collect::<Vec<(LayerNodeIdentifier, f64)>>();
				} else {
					// deselect all the layer and store the clicked layer for repeat and dragging

					responses.add(NodeGraphMessage::SelectedNodesSet { nodes: vec![clicked_layer.to_node()] });
					let (_angle_offset, radius, _count) = extract_circular_repeat_parameters(Some(clicked_layer), document).unwrap_or((0.0, 0.0, 6));
					tool_data.clicked_layer_radius = (clicked_layer, radius);
					tool_data.layers_dragging = vec![(clicked_layer, radius)];
				}
				tool_data.drag_start = input.mouse.position;
				tool_data.initial_center = viewport.transform_point2(DVec2::ZERO);

				OperationToolFsmState::Drawing
			}
			(OperationToolFsmState::Drawing, OperationToolMessage::DragStop) => {
				if tool_data.drag_start.distance(input.mouse.position) < 5. {
					responses.add(DocumentMessage::AbortTransaction);
				};
				tool_data.cleanup();
				responses.add(DocumentMessage::EndTransaction);
				OperationToolFsmState::Ready
			}
			(OperationToolFsmState::Drawing, OperationToolMessage::PointerMove) => {
				// Don't add the repeat node unless dragging more that 5 px
				if tool_data.drag_start.distance(input.mouse.position) < 5. {
					return self;
				};

				let (_clicked_layer, clicked_radius) = tool_data.clicked_layer_radius;
				let viewport = document.metadata().transform_to_viewport(tool_data.clicked_layer_radius.0);
				let sign = (input.mouse.position - tool_data.initial_center).dot(viewport.transform_vector2(DVec2::Y)).signum();
				let delta = document
					.metadata()
					.downstream_transform_to_viewport(tool_data.clicked_layer_radius.0)
					.inverse()
					.transform_vector2(input.mouse.position - tool_data.initial_center)
					.length() * sign;

				for (layer, initial_radius) in &tool_data.layers_dragging {
					let new_radius = if initial_radius.signum() == clicked_radius.signum() {
						*initial_radius + delta
					} else {
						*initial_radius + delta.signum() * -1. * delta.abs()
					};

					responses.add(GraphOperationMessage::CircularRepeatSet {
						layer: *layer,
						angle: 0.,
						radius: new_radius,
						count: 6,
					});
				}
				responses.add(NodeGraphMessage::RunDocumentGraph);

				OperationToolFsmState::Drawing
			}
			(_, OperationToolMessage::PointerMove) => {
				responses.add(OverlaysMessage::Draw);
				self
			}

			(OperationToolFsmState::Drawing, OperationToolMessage::PointerOutsideViewport) => OperationToolFsmState::Drawing,
			(state, OperationToolMessage::PointerOutsideViewport) => state,
			(OperationToolFsmState::Drawing, OperationToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				OperationToolFsmState::Ready
			}
			(_, OperationToolMessage::WorkingColorChanged) => self,
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			OperationToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Draw Spline"),
				HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
			])]),
			OperationToolFsmState::Drawing => HintData(vec![
				HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()]),
				HintGroup(vec![HintInfo::mouse(MouseMotion::Lmb, "Extend Spline")]),
				HintGroup(vec![HintInfo::keys([Key::Enter], "End Spline")]),
			]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OperationType {
	#[default]
	CircularRepeat = 0,
	Repeat = 1,
}
