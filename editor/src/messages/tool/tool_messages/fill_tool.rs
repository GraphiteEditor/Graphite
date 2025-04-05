use graph_craft::document::value::TaggedValue;
use graphene_std::vector::style::Fill;

use super::tool_prelude::*;
use crate::messages::portfolio::document::graph_operation::transform_utils::get_current_transform;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;

#[derive(Default)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
}

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FillToolMessage {
	// Standard messages
	Abort,

	// Tool-specific messagesty-dlp
	PointerUp,
	FillPrimaryColor,
	FillSecondaryColor,
}

impl ToolMetadata for FillTool {
	fn icon_name(&self) -> String {
		"GeneralFillTool".into()
	}
	fn tooltip(&self) -> String {
		"Fill Tool".into()
	}
	fn tool_type(&self) -> crate::messages::tool::utility_types::ToolType {
		ToolType::Fill
	}
}

impl LayoutHolder for FillTool {
	fn layout(&self) -> Layout {
		Layout::WidgetLayout(WidgetLayout::default())
	}
}

impl<'a> MessageHandler<ToolMessage, &mut ToolActionHandlerData<'a>> for FillTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, tool_data: &mut ToolActionHandlerData<'a>) {
		let tool_datas = &mut RasterFillToolData::default();
		self.fsm_state.process_event(message, tool_datas, tool_data, &(), responses, true);
	}
	fn actions(&self) -> ActionList {
		match self.fsm_state {
			FillToolFsmState::Ready => actions!(FillToolMessageDiscriminant;
				FillPrimaryColor,
				FillSecondaryColor,
			),
			FillToolFsmState::Filling => actions!(FillToolMessageDiscriminant;
				PointerUp,
				Abort,
			),
		}
	}
}

impl ToolTransition for FillTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap {
			tool_abort: Some(FillToolMessage::Abort.into()),
			..Default::default()
		}
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FillToolFsmState {
	#[default]
	Ready,
	// Implemented as a fake dragging state that can be used to abort unwanted fills
	Filling,
}

#[derive(Clone, Debug, Default)]
struct RasterFillToolData {
	fills: Vec<Fill>,
	start_pos: Vec<DVec2>,
	layer: Option<LayerNodeIdentifier>,
	similarity_threshold: f64,
}

impl RasterFillToolData {
	fn load_existing_fills(&mut self, document: &mut DocumentMessageHandler, layer_identifier: LayerNodeIdentifier) -> Option<LayerNodeIdentifier> {
		let node_graph_layer = NodeGraphLayer::new(layer_identifier, &mut document.network_interface);
		let existing_fills = node_graph_layer.find_node_inputs("Raster Fill");
		if let Some(existing_fills) = existing_fills {
			let fills = if let Some(TaggedValue::FillCache(fills)) = existing_fills[1].as_value() {
				fills.clone()
			} else {
				vec![]
			};
			let start_pos = if let Some(TaggedValue::VecDVec2(start_pos)) = existing_fills[2].as_value() {
				start_pos.clone()
			} else {
				vec![]
			};
			let similarity_threshold = if let Some(TaggedValue::F64(similarity_threshold)) = existing_fills[3].as_value() {
				*similarity_threshold
			} else {
				1.
			};
			self.fills = fills;
			self.start_pos = start_pos;
			self.layer = Some(layer_identifier);
			self.similarity_threshold = similarity_threshold;
		}
		self.similarity_threshold = 1.;
		None
	}
}

impl Fsm for FillToolFsmState {
	type ToolData = RasterFillToolData;
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, tool_data: &mut Self::ToolData, handler_data: &mut ToolActionHandlerData, _tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = handler_data;

		let ToolMessage::Fill(event) = event else { return self };
		match (self, event) {
			(FillToolFsmState::Ready, color_event) => {
				let Some(layer_identifier) = document.click(input) else {
					return self;
				};
				let fill = match color_event {
					FillToolMessage::FillPrimaryColor => Fill::Solid(global_tool_data.primary_color.to_gamma_srgb()),
					FillToolMessage::FillSecondaryColor => Fill::Solid(global_tool_data.secondary_color.to_gamma_srgb()),
					_ => return self,
				};

				responses.add(DocumentMessage::AddTransaction);
				// If the layer is a raster layer, use the raster fill functionality
				if NodeGraphLayer::is_raster_layer(layer_identifier, &mut document.network_interface) {
					// Try to load existing fills for this layer
					tool_data.load_existing_fills(document, layer_identifier);

					// Get position in layer space
					let layer_pos = document
						.network_interface
						.document_metadata()
						.downstream_transform_to_viewport(layer_identifier)
						.inverse()
						.transform_point2(input.mouse.position);

					let node_graph_layer = NodeGraphLayer::new(layer_identifier, &mut document.network_interface);
					if let Some(transform_inputs) = node_graph_layer.find_node_inputs("Transform") {
						let image_transform = get_current_transform(transform_inputs);
						let image_local_pos = image_transform.inverse().transform_point2(layer_pos);
						// Store the fill in our tool data with its position
						tool_data.fills.push(fill.clone());
						tool_data.start_pos.push(image_local_pos);
					}

					// Send the fill operation message
					responses.add(GraphOperationMessage::RasterFillSet {
						layer: layer_identifier,
						fills: tool_data.fills.clone(),
						start_pos: tool_data.start_pos.clone(),
						similarity_threshold: tool_data.similarity_threshold,
					});
				} else {
					// For vector layers, use the existing functionality
					responses.add(GraphOperationMessage::FillSet { layer: layer_identifier, fill });
				}

				FillToolFsmState::Filling
			}
			(FillToolFsmState::Filling, FillToolMessage::PointerUp) => {
				// Clear the fills data when we're done
				tool_data.fills.clear();
				tool_data.start_pos.clear();
				FillToolFsmState::Ready
			}
			(FillToolFsmState::Filling, FillToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);

				FillToolFsmState::Ready
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FillToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Fill with Primary"),
				HintInfo::keys([Key::Shift], "Fill with Secondary").prepend_plus(),
			])]),
			FillToolFsmState::Filling => HintData(vec![HintGroup(vec![HintInfo::mouse(MouseMotion::Rmb, ""), HintInfo::keys([Key::Escape], "Cancel").prepend_slash()])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}

#[cfg(test)]
mod test_fill {
	pub use crate::test_utils::test_prelude::*;
	use graphene_core::vector::fill;
	use graphene_std::vector::style::Fill;

	async fn get_fills(editor: &mut EditorTestUtils) -> Vec<Fill> {
		let instrumented = editor.eval_graph().await;

		instrumented.grab_all_input::<fill::FillInput<Fill>>(&editor.runtime).collect()
	}

	#[tokio::test]
	async fn ignore_artboard() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Artboard, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;
		assert!(get_fills(&mut editor,).await.is_empty());
	}

	#[tokio::test]
	async fn ignore_raster() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.create_raster_image(Image::new(100, 100, Color::WHITE), Some((0., 0.))).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;
		assert!(get_fills(&mut editor,).await.is_empty());
	}

	#[tokio::test]
	async fn primary() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		editor.select_primary_color(Color::GREEN).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::empty()).await;
		let fills = get_fills(&mut editor).await;
		assert_eq!(fills.len(), 1);
		assert_eq!(fills[0].as_solid().unwrap().to_rgba8_srgb(), Color::GREEN.to_rgba8_srgb());
	}

	#[tokio::test]
	async fn secondary() {
		let mut editor = EditorTestUtils::create();
		editor.new_document().await;
		editor.drag_tool(ToolType::Rectangle, 0., 0., 100., 100., ModifierKeys::empty()).await;
		let color = Color::YELLOW;
		editor.handle_message(ToolMessage::SelectSecondaryColor { color }).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::SHIFT).await;
		let fills = get_fills(&mut editor).await;
		assert_eq!(fills.len(), 1);
		assert_eq!(fills[0].as_solid().unwrap().to_rgba8_srgb(), color.to_rgba8_srgb());
	}
}
