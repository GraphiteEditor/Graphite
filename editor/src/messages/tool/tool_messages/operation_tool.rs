use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::gizmos::gizmo_manager::GizmoManager;
use crate::messages::tool::common_functionality::operations::circular_repeat::{CircularRepeatOperation, CircularRepeatOperationData};
use crate::messages::tool::common_functionality::shapes::shape_utility::GizmoContext;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum OperationType {
	#[default]
	CircularRepeat = 0,
	Repeat,
}

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
	IncreaseCount,
	DecreaseCount,
	Confirm,
	DragStart,
	DragStop,
	PointerMove,
	PointerOutsideViewport,
	Undo,
	UpdateOptions { options: OperationOptionsUpdate },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OperationToolFsmState {
	#[default]
	Ready,
	ModifyingGizmo,
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
		MenuListEntry::new("Circular Repeat").label("Circular Repeat").on_commit(move |_| {
			OperationToolMessage::UpdateOptions {
				options: OperationOptionsUpdate::OperationType(OperationType::CircularRepeat),
			}
			.into()
		}),
		MenuListEntry::new("Repeat").label("Repeat").on_commit(move |_| {
			OperationToolMessage::UpdateOptions {
				options: OperationOptionsUpdate::OperationType(OperationType::Repeat),
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
			OperationToolFsmState::Drawing | OperationToolFsmState::ModifyingGizmo => actions!(OperationToolMessageDiscriminant;
				DragStop,
				PointerMove,
				Confirm,
				Abort,
				IncreaseCount,
				DecreaseCount,
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
pub struct OperationToolData {
	pub drag_start: DVec2,
	pub circular_operation_data: CircularRepeatOperationData,
	gizmo_manager: GizmoManager,
}

impl OperationToolData {
	fn cleanup(&mut self) {
		CircularRepeatOperation::cleanup(self);
	}

	fn common_overlays(&self, document: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler, overlay_context: &mut OverlayContext) {
		for layer in document.network_interface.selected_nodes().selected_visible_and_unlocked_layers(&document.network_interface) {
			outline_layer(layer, document, overlay_context);

			let Some(hovered_layer) = document.click(input) else { continue };
			if hovered_layer != layer {
				outline_layer(hovered_layer, document, overlay_context);
			}
		}
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
		tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext { document, input, shape_editor, .. } = tool_action_data;

		let mut ctx = GizmoContext {
			document,
			input,
			responses,
			shape_editor,
		};

		let ToolMessage::Operation(event) = event else { return self };
		match (self, event) {
			(_, OperationToolMessage::Overlays { context: mut overlay_context }) => {
				if matches!(self, OperationToolFsmState::Ready) {
					tool_data.gizmo_manager.handle_operation_actions(input.mouse.position, &mut ctx);
					tool_data.gizmo_manager.overlays(input.mouse.position, &mut ctx, &mut overlay_context);
					CircularRepeatOperation::overlays(&self, tool_data, document, input, &mut overlay_context);
				}

				if matches!(self, OperationToolFsmState::ModifyingGizmo) {
					tool_data.gizmo_manager.overlays(input.mouse.position, &mut ctx, &mut overlay_context);
				}

				tool_data.common_overlays(document, input, &mut overlay_context);

				self
			}
			(OperationToolFsmState::Ready, OperationToolMessage::DragStart) => {
				if tool_data.gizmo_manager.handle_click() {
					tool_data.drag_start = input.mouse.position;
					return OperationToolFsmState::ModifyingGizmo;
				}
				match tool_options.operation_type {
					OperationType::CircularRepeat => {
						CircularRepeatOperation::create_node(tool_data, document, responses, input);
					}
					OperationType::Repeat => {}
				}

				OperationToolFsmState::Drawing
			}
			(OperationToolFsmState::Drawing | OperationToolFsmState::ModifyingGizmo, OperationToolMessage::DragStop) => {
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

				match tool_options.operation_type {
					OperationType::CircularRepeat => {
						CircularRepeatOperation::update_shape(tool_data, document, responses, input);
					}
					OperationType::Repeat => {}
				}

				OperationToolFsmState::Drawing
			}
			(OperationToolFsmState::ModifyingGizmo, OperationToolMessage::PointerMove) => {
				// Don't add the repeat node unless dragging more that 5 px
				tool_data.gizmo_manager.handle_update(tool_data.drag_start, &mut ctx);

				OperationToolFsmState::ModifyingGizmo
			}
			(OperationToolFsmState::Drawing, OperationToolMessage::IncreaseCount) => {
				match tool_options.operation_type {
					OperationType::CircularRepeat => CircularRepeatOperation::increase_decrease_count(tool_data, true, document, responses),
					_ => {}
				}
				self
			}
			(OperationToolFsmState::Drawing, OperationToolMessage::DecreaseCount) => {
				match tool_options.operation_type {
					OperationType::CircularRepeat => CircularRepeatOperation::increase_decrease_count(tool_data, false, document, responses),
					_ => {}
				}
				self
			}
			(_, OperationToolMessage::PointerMove) => {
				responses.add(OverlaysMessage::Draw);
				self
			}

			(OperationToolFsmState::Drawing, OperationToolMessage::PointerOutsideViewport) => OperationToolFsmState::Drawing,
			(state, OperationToolMessage::PointerOutsideViewport) => state,
			(OperationToolFsmState::Drawing | OperationToolFsmState::ModifyingGizmo, OperationToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				OperationToolFsmState::Ready
			}
			(_, OperationToolMessage::WorkingColorChanged) => self,
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			OperationToolFsmState::Ready | OperationToolFsmState::ModifyingGizmo => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Draw Spline"),
				HintInfo::keys([Key::Shift], "Append to Selected Layer").prepend_plus(),
			])]),
			_ => HintData(vec![
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

fn outline_layer(layer: LayerNodeIdentifier, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
	let Some(vector) = document.network_interface.compute_modified_vector(layer) else { return };
	let viewport = document.metadata().transform_to_viewport(layer);
	overlay_context.outline_vector(&vector, viewport);
}
