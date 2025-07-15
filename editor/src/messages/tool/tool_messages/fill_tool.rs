use super::tool_prelude::*;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::tool::common_functionality::graph_modification_utils::NodeGraphLayer;
use graphene_std::vector::style::Fill;

#[derive(Default, ExtractField)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
}

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Clone, Debug, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum FillToolMessage {
	// Standard messages
	Abort,
	WorkingColorChanged,
	Overlays(OverlayContext),

	// Tool-specific messages
	PointerMove,
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

#[message_handler_data]
impl<'a> MessageHandler<ToolMessage, &mut ToolActionMessageContext<'a>> for FillTool {
	fn process_message(&mut self, message: ToolMessage, responses: &mut VecDeque<Message>, context: &mut ToolActionMessageContext<'a>) {
		self.fsm_state.process_event(message, &mut (), context, &(), responses, true);
	}
	fn actions(&self) -> ActionList {
		match self.fsm_state {
			FillToolFsmState::Ready => actions!(FillToolMessageDiscriminant;
				FillPrimaryColor,
				FillSecondaryColor,
				PointerMove,
			),
			FillToolFsmState::Filling => actions!(FillToolMessageDiscriminant;
				PointerMove,
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
			working_color_changed: Some(FillToolMessage::WorkingColorChanged.into()),
			overlay_provider: Some(|overlay_context| FillToolMessage::Overlays(overlay_context).into()),
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

impl Fsm for FillToolFsmState {
	type ToolData = ();
	type ToolOptions = ();

	fn transition(
		self,
		event: ToolMessage,
		_tool_data: &mut Self::ToolData,
		handler_data: &mut ToolActionMessageContext,
		_tool_options: &Self::ToolOptions,
		responses: &mut VecDeque<Message>,
	) -> Self {
		let ToolActionMessageContext {
			document, global_tool_data, input, ..
		} = handler_data;

		let ToolMessage::Fill(event) = event else { return self };
		match (self, event) {
			(_, FillToolMessage::Overlays(mut overlay_context)) => {
				// Choose the working color to preview
				let use_secondary = input.keyboard.get(Key::Shift as usize);
				let preview_color = if use_secondary { global_tool_data.secondary_color } else { global_tool_data.primary_color };

				// Get the layer the user is hovering over
				if let Some(layer) = document.click(input) {
					overlay_context.fill_path_pattern(document.metadata().layer_outline(layer), document.metadata().transform_to_viewport(layer), &preview_color);
				}

				self
			}
			(_, FillToolMessage::PointerMove | FillToolMessage::WorkingColorChanged) => {
				// Generate the hover outline
				responses.add(OverlaysMessage::Draw);
				self
			}
			(FillToolFsmState::Ready, color_event) => {
				let Some(layer_identifier) = document.click(input) else {
					return self;
				};
				// If the layer is a raster layer, don't fill it, wait till the flood fill tool is implemented
				if NodeGraphLayer::is_raster_layer(layer_identifier, &mut document.network_interface) {
					return self;
				}
				let fill = match color_event {
					FillToolMessage::FillPrimaryColor => Fill::Solid(global_tool_data.primary_color.to_gamma_srgb()),
					FillToolMessage::FillSecondaryColor => Fill::Solid(global_tool_data.secondary_color.to_gamma_srgb()),
					_ => return self,
				};

				responses.add(DocumentMessage::AddTransaction);
				responses.add(GraphOperationMessage::FillSet { layer: layer_identifier, fill });

				FillToolFsmState::Filling
			}
			(FillToolFsmState::Filling, FillToolMessage::PointerUp) => FillToolFsmState::Ready,
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
	use graphene_std::vector::fill;
	use graphene_std::vector::style::Fill;

	async fn get_fills(editor: &mut EditorTestUtils) -> Vec<Fill> {
		let instrumented = match editor.eval_graph().await {
			Ok(instrumented) => instrumented,
			Err(e) => panic!("Failed to evaluate graph: {e}"),
		};

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
		editor.select_secondary_color(Color::YELLOW).await;
		editor.click_tool(ToolType::Fill, MouseKeys::LEFT, DVec2::new(2., 2.), ModifierKeys::SHIFT).await;
		let fills = get_fills(&mut editor).await;
		assert_eq!(fills.len(), 1);
		assert_eq!(fills[0].as_solid().unwrap().to_rgba8_srgb(), Color::YELLOW.to_rgba8_srgb());
	}
}
