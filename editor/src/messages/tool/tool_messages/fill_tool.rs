use super::tool_prelude::*;

use graphene_core::vector::style::Fill;

#[derive(Default)]
pub struct FillTool {
	fsm_state: FillToolFsmState,
}

#[impl_message(Message, ToolMessage, Fill)]
#[derive(PartialEq, Eq, Clone, Debug, Hash, Serialize, Deserialize, specta::Type)]
pub enum FillToolMessage {
	// Standard messages
	Abort,

	// Tool-specific messages
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
		self.fsm_state.process_event(message, &mut (), tool_data, &(), responses, true);
	}
	fn actions(&self) -> ActionList {
		use FillToolFsmState::*;

		match self.fsm_state {
			Ready => actions!(FillToolMessageDiscriminant;
				FillPrimaryColor,
				FillSecondaryColor,
			),
			Filling => actions!(FillToolMessageDiscriminant;
				PointerUp,
				Abort,
			),
		}
	}
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
	// Implemented as a fake dragging state that can be used to abort unwanted fills
	Filling,
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
		let Some(layer_identifier) = document.click(input.mouse.position, &document.network) else {
			return self;
		};
		match (self, event) {
			(FillToolFsmState::Ready, color_event) => {
				// TODO: Use a match statement here instead of if-else
				let color = if color_event == FillToolMessage::FillPrimaryColor {
					global_tool_data.primary_color
				} else {
					global_tool_data.secondary_color
				};
				let fill = Fill::Solid(color);

				responses.add(DocumentMessage::StartTransaction);
				responses.add(GraphOperationMessage::FillSet { layer: layer_identifier, fill });
				responses.add(DocumentMessage::CommitTransaction);

				FillToolFsmState::Filling
			}
			(FillToolFsmState::Filling, FillToolMessage::PointerUp) => FillToolFsmState::Ready,
			(FillToolFsmState::Filling, FillToolMessage::Abort) => {
				responses.add(DocumentMessage::AbortTransaction);
				return FillToolFsmState::Ready;
			}
			_ => self,
		}
	}

	fn update_hints(&self, responses: &mut VecDeque<Message>) {
		let hint_data = match self {
			FillToolFsmState::Ready => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Lmb, "Fill with Primary"),
				HintInfo::keys_and_mouse([Key::Shift], MouseMotion::Lmb, "Fill with Secondary"),
			])]),
			FillToolFsmState::Filling => HintData(vec![HintGroup(vec![
				HintInfo::mouse(MouseMotion::Rmb, ""),
				HintInfo::keys([Key::Escape], "Cancel").prepend_slash(),
			])]),
		};

		responses.add(FrontendMessage::UpdateInputHints { hint_data });
	}

	fn update_cursor(&self, responses: &mut VecDeque<Message>) {
		responses.add(FrontendMessage::UpdateMouseCursor { cursor: MouseCursorIcon::Default });
	}
}
