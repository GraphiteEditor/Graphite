use super::tool_prelude::*;
use document_legacy::layers::style::Fill;

#[derive(Default)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
}

#[remain::sorted]
#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum FillToolMessage {
	// Tool-specific messages
	LeftPointerDown,
	RightPointerDown,
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
		self.fsm_state.process_event(message, &mut (), tool_data, &(), responses, true);
	}

	advertise_actions!(FillToolMessageDiscriminant;
		LeftPointerDown,
		RightPointerDown,
	);
}

impl ToolTransition for FillTool {
	fn event_to_message_map(&self) -> EventToMessageMap {
		EventToMessageMap::default()
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FillToolFsmState {
	#[default]
	Ready,
}

impl Fsm for FillToolFsmState {
	type ToolData = ();
	type ToolOptions = ();

	fn transition(self, event: ToolMessage, _tool_data: &mut Self::ToolData, handler_data: &mut ToolActionHandlerData, _tool_options: &Self::ToolOptions, responses: &mut VecDeque<Message>) -> Self {
		let ToolActionHandlerData {
			document, global_tool_data, input, ..
		} = handler_data;

		let ToolMessage::Fill(event) = event else {
			return self;
		};
		let Some((layer_identifier, _)) = document.document_legacy.metadata.click(input.mouse.position) else {
			return self;
		};
		let layer = layer_identifier.to_path();

		let color = match event {
			FillToolMessage::LeftPointerDown => global_tool_data.primary_color,
			FillToolMessage::RightPointerDown => global_tool_data.secondary_color,
		};
		let fill = Fill::Solid(color);

		responses.add(DocumentMessage::StartTransaction);
		responses.add(DocumentMessage::SetSelectedLayers {
			replacement_selected_layers: vec![layer.clone()],
		});
		responses.add(GraphOperationMessage::FillSet { layer, fill });
		responses.add(DocumentMessage::CommitTransaction);

		FillToolFsmState::Ready
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FillToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Fill with Primary"),
				HintInfo::mouse(MouseMotion::Rmb, "Fill with Secondary"),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
