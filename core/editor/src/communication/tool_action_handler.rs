use document_core::{color::Color, DocumentResponse, LayerId};

use super::{Action, InputPreprocessor, MessageHandler, Operation, Response};
use crate::{events::ToolResponse, tools::ToolType, SvgDocument};
use crate::{
	tools::{DocumentToolData, ToolActionHandlerData},
	EditorError,
};

use strum_macros::{AsRefStr, Display, EnumDiscriminants, EnumIter, EnumString};

#[derive(Debug, Clone, Display, AsRefStr, EnumDiscriminants, EnumIter, EnumString)]
pub enum ToolMessage {
	SelectTool(ToolType),
	SelectPrimaryColor(Color),
	SelectSecondaryColor(Color),
	Undo,
	Redo,
	Save,
}

#[derive(Debug, Default, Clone)]
pub struct ToolActionHandler {}

impl MessageHandler<ToolMessage, &mut SvgDocument> for ToolActionHandler {
	fn process_action(&mut self, action: ToolMessage, document: &mut SvgDocument, responses: &mut Vec<Response>) {}
	actions_fn!(Action::Undo, Action::DeleteLayer(vec![]), Action::AddFolder(vec![]));
}
