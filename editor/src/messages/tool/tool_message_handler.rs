use super::common_functionality::shape_editor::ShapeState;
use super::utility_types::{tool_message_to_tool_type, ToolActionHandlerData, ToolFsmState};
use crate::application::generate_uuid;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayProvider;
use crate::messages::portfolio::utility_types::PersistentData;
use crate::messages::prelude::*;
use crate::messages::tool::utility_types::ToolType;
use crate::node_graph_executor::NodeGraphExecutor;

use graphene_core::raster::color::Color;

const ARTBOARD_OVERLAY_PROVIDER: OverlayProvider = |context| DocumentMessage::DrawArtboardOverlays(context).into();

pub struct ToolMessageData<'a> {
	pub document_id: DocumentId,
	pub document: &'a mut DocumentMessageHandler,
	pub input: &'a InputPreprocessorMessageHandler,
	pub persistent_data: &'a PersistentData,
	pub node_graph: &'a NodeGraphExecutor,
}

#[derive(Debug, Default)]
pub struct ToolMessageHandler {
	pub tool_state: ToolFsmState,
	pub transform_layer_handler: TransformLayerMessageHandler,
	pub shape_editor: ShapeState,
	pub tool_is_active: bool,
}

impl MessageHandler<ToolMessage, ToolMessageData<'_>> for ToolMessageHandler {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, data: ToolMessageData) {
		let ToolMessageData {
			document_id,
			document,
			input,
			persistent_data,
			node_graph,
		} = data;
		let font_cache = &persistent_data.font_cache;

		match message {
			// Messages
			ToolMessage::TransformLayer(message) => self
				.transform_layer_handler
				.process_message(message, responses, (document, input, &self.tool_state.tool_data, &mut self.shape_editor)),

			ToolMessage::ActivateToolSelect => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Select }),
			ToolMessage::ActivateToolArtboard => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Artboard }),
			ToolMessage::ActivateToolNavigate => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Navigate }),
			ToolMessage::ActivateToolEyedropper => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Eyedropper }),
			ToolMessage::ActivateToolText => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Text }),
			ToolMessage::ActivateToolFill => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Fill }),
			ToolMessage::ActivateToolGradient => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Gradient }),

			ToolMessage::ActivateToolPath => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Path }),
			ToolMessage::ActivateToolPen => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Pen }),
			ToolMessage::ActivateToolFreehand => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Freehand }),
			ToolMessage::ActivateToolSpline => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Spline }),
			ToolMessage::ActivateToolLine => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Line }),
			ToolMessage::ActivateToolRectangle => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Rectangle }),
			ToolMessage::ActivateToolEllipse => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Ellipse }),
			ToolMessage::ActivateToolPolygon => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Polygon }),

			ToolMessage::ActivateToolBrush => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Brush }),
			ToolMessage::ActivateToolImaginate => responses.add_front(ToolMessage::ActivateTool { tool_type: ToolType::Imaginate }),

			ToolMessage::ActivateTool { tool_type } => {
				let tool_data = &mut self.tool_state.tool_data;
				let old_tool = tool_data.active_tool_type;

				// Do nothing if switching to the same tool
				if self.tool_is_active && tool_type == old_tool {
					return;
				}
				self.tool_is_active = true;

				// Send the old and new tools a transition to their FSM Abort states
				let mut send_abort_to_tool = |tool_type, update_hints_and_cursor: bool| {
					if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
						let mut data = ToolActionHandlerData {
							document,
							document_id,
							global_tool_data: &self.tool_state.document_tool_data,
							input,
							font_cache,
							shape_editor: &mut self.shape_editor,
							node_graph,
						};

						if let Some(tool_abort_message) = tool.event_to_message_map().tool_abort {
							tool.process_message(tool_abort_message, responses, &mut data);
						}

						if update_hints_and_cursor {
							if self.transform_layer_handler.is_transforming() {
								self.transform_layer_handler.hints(responses);
							} else {
								tool.process_message(ToolMessage::UpdateHints, responses, &mut data);
							}
							tool.process_message(ToolMessage::UpdateCursor, responses, &mut data);
						}
					}
				};
				send_abort_to_tool(tool_type, true);
				send_abort_to_tool(old_tool, false);

				// Unsubscribe old tool from the broadcaster
				tool_data.tools.get(&tool_type).unwrap().deactivate(responses);

				// Store the new active tool
				tool_data.active_tool_type = tool_type;

				// Subscribe new tool
				tool_data.tools.get(&tool_type).unwrap().activate(responses);

				// Send the SelectionChanged message to the active tool, this will ensure the selection is updated
				responses.add(BroadcastEvent::SelectionChanged);

				// Update the working colors for the active tool
				responses.add(BroadcastEvent::WorkingColorChanged);

				// Send tool options to the frontend
				responses.add(ToolMessage::RefreshToolOptions);

				// Notify the frontend about the new active tool to be displayed
				tool_data.send_layout(responses, LayoutTarget::ToolShelf);
			}
			ToolMessage::DeactivateTools => {
				let tool_data = &mut self.tool_state.tool_data;
				tool_data.tools.get(&tool_data.active_tool_type).unwrap().deactivate(responses);

				// Unsubscribe the transform layer to selection change events
				let message = Box::new(TransformLayerMessage::SelectionChanged.into());
				let on = BroadcastEvent::SelectionChanged;
				responses.add(BroadcastMessage::UnsubscribeEvent { message, on });

				responses.add(OverlaysMessage::RemoveProvider(ARTBOARD_OVERLAY_PROVIDER));

				responses.add(FrontendMessage::UpdateInputHints { hint_data: Default::default() });
				responses.add(FrontendMessage::UpdateMouseCursor { cursor: Default::default() });

				self.tool_is_active = false;
			}
			ToolMessage::InitTools => {
				// Subscribe the transform layer to selection change events
				responses.add(BroadcastMessage::SubscribeEvent {
					on: BroadcastEvent::SelectionChanged,
					send: Box::new(TransformLayerMessage::SelectionChanged.into()),
				});

				self.tool_is_active = true;

				let tool_data = &mut self.tool_state.tool_data;
				let document_data = &self.tool_state.document_tool_data;
				let active_tool = &tool_data.active_tool_type;

				// Subscribe tool to broadcast messages
				tool_data.tools.get(active_tool).unwrap().activate(responses);

				// Register initial properties
				tool_data.tools.get(active_tool).unwrap().send_layout(responses, LayoutTarget::ToolOptions);

				// Notify the frontend about the initial active tool
				tool_data.send_layout(responses, LayoutTarget::ToolShelf);

				// Notify the frontend about the initial working colors
				document_data.update_working_colors(responses);

				let mut data = ToolActionHandlerData {
					document,
					document_id,
					global_tool_data: &self.tool_state.document_tool_data,
					input,
					font_cache,
					shape_editor: &mut self.shape_editor,
					node_graph,
				};

				// Set initial hints and cursor
				tool_data.active_tool_mut().process_message(ToolMessage::UpdateHints, responses, &mut data);
				tool_data.active_tool_mut().process_message(ToolMessage::UpdateCursor, responses, &mut data);

				responses.add(OverlaysMessage::AddProvider(ARTBOARD_OVERLAY_PROVIDER));
			}
			ToolMessage::PreUndo => {
				let tool_data = &mut self.tool_state.tool_data;
				if tool_data.active_tool_type != ToolType::Pen {
					responses.add(BroadcastEvent::ToolAbort);
				}
			}
			ToolMessage::Redo => {
				let tool_data = &mut self.tool_state.tool_data;
				if tool_data.active_tool_type == ToolType::Pen {
					responses.add(PenToolMessage::Redo);
				}
			}
			ToolMessage::RefreshToolOptions => {
				let tool_data = &mut self.tool_state.tool_data;
				tool_data.tools.get(&tool_data.active_tool_type).unwrap().send_layout(responses, LayoutTarget::ToolOptions);
			}
			ToolMessage::ResetColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				document_data.primary_color = Color::BLACK;
				document_data.secondary_color = Color::WHITE;

				document_data.update_working_colors(responses); // TODO: Make this an event
			}
			ToolMessage::SelectPrimaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.primary_color = color;

				self.tool_state.document_tool_data.update_working_colors(responses); // TODO: Make this an event
			}
			ToolMessage::SelectRandomPrimaryColor => {
				// Select a random primary color (rgba) based on an UUID
				let document_data = &mut self.tool_state.document_tool_data;

				let random_number = generate_uuid();
				let r = (random_number >> 16) as u8;
				let g = (random_number >> 8) as u8;
				let b = random_number as u8;
				let random_color = Color::from_rgba8_srgb(r, g, b, 255);
				document_data.primary_color = random_color;

				document_data.update_working_colors(responses); // TODO: Make this an event
			}
			ToolMessage::SelectSecondaryColor { color } => {
				let document_data = &mut self.tool_state.document_tool_data;
				document_data.secondary_color = color;

				document_data.update_working_colors(responses); // TODO: Make this an event
			}
			ToolMessage::SwapColors => {
				let document_data = &mut self.tool_state.document_tool_data;

				std::mem::swap(&mut document_data.primary_color, &mut document_data.secondary_color);

				document_data.update_working_colors(responses); // TODO: Make this an event
			}
			ToolMessage::Undo => {
				let tool_data = &mut self.tool_state.tool_data;
				if tool_data.active_tool_type == ToolType::Pen {
					responses.add(PenToolMessage::Undo);
				}
			}

			// Sub-messages
			tool_message => {
				let tool_type = match &tool_message {
					ToolMessage::UpdateCursor | ToolMessage::UpdateHints => self.tool_state.tool_data.active_tool_type,
					tool_message => tool_message_to_tool_type(tool_message),
				};
				let tool_data = &mut self.tool_state.tool_data;

				if let Some(tool) = tool_data.tools.get_mut(&tool_type) {
					if tool_type == tool_data.active_tool_type {
						let mut data = ToolActionHandlerData {
							document,
							document_id,
							global_tool_data: &self.tool_state.document_tool_data,
							input,
							font_cache,
							shape_editor: &mut self.shape_editor,
							node_graph,
						};
						if matches!(tool_message, ToolMessage::UpdateHints) {
							if self.transform_layer_handler.is_transforming() {
								self.transform_layer_handler.hints(responses);
							} else {
								tool.process_message(ToolMessage::UpdateHints, responses, &mut data)
							}
						} else {
							tool.process_message(tool_message, responses, &mut data);
						}
					}
				}
			}
		}
	}

	fn actions(&self) -> ActionList {
		let mut list = actions!(ToolMessageDiscriminant;
			ActivateToolSelect,
			ActivateToolArtboard,
			ActivateToolNavigate,
			ActivateToolEyedropper,
			ActivateToolText,
			ActivateToolFill,
			ActivateToolGradient,

			ActivateToolPath,
			ActivateToolPen,
			ActivateToolFreehand,
			ActivateToolSpline,
			ActivateToolLine,
			ActivateToolRectangle,
			ActivateToolEllipse,
			ActivateToolPolygon,

			ActivateToolBrush,
			ActivateToolImaginate,

			SelectRandomPrimaryColor,
			ResetColors,
			SwapColors,
			Undo,
		);
		list.extend(self.tool_state.tool_data.active_tool().actions());
		list.extend(self.transform_layer_handler.actions());

		list
	}
}
