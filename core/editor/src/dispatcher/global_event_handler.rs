use crate::{document::Document, events::ToolResponse, tools::ToolType, Color};

use super::{input_manager::InputPreprocessor, Action, ActionHandler, ActionList, Operation, Response};
use crate::tools::ToolFsmState;

#[derive(Debug)]
pub struct GlobalEventHandler {
	documents: Vec<Document>,
	active_document: usize,
	tool_state: ToolFsmState,
	actions: Vec<(String, Action)>,
}

impl GlobalEventHandler {
	pub fn new() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
			tool_state: ToolFsmState::default(),
			actions: Vec::new(),
		}
	}
	fn current_actions<'a>() -> ActionList<'a> {
		use Action::*;
		actions!(
			SelectDocument(0),
			SelectTool(ToolType::Select),
			LogInfo,
			LogDebug,
			LogTrace,
			SelectPrimaryColor(Color::WHITE),
			SelectSecondaryColor(Color::BLACK)
		)
	}
	fn update_actions(&mut self) {
		self.actions.clear();
		self.actions.extend_from_slice(Self::current_actions());
		let tool_name = format!(".tool.{}", self.tool_state.tool_data.active_tool_type);
		if let Ok(tool) = self.tool_state.tool_data.active_tool() {
			self.actions.extend(tool.actions().iter().map(|(name, action)| (format!("{}{}", tool_name, name), action.clone())));
		}
		let document = &self.documents[self.active_document];
		self.actions
			.extend(document.handler.actions().iter().map(|(name, action)| (format!(".document{}", name), action.clone())));
	}
}

impl ActionHandler<()> for GlobalEventHandler {
	fn process_action(&mut self, _data: (), input: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		let mut consumed = true;

		// process action before passing them further down
		use Action::*;
		match action {
			SelectDocument(id) => {
				self.active_document = *id;
				self.update_actions()
			}
			SelectTool(tool) => {
				self.tool_state.tool_data.active_tool_type = *tool;
				responses.push(ToolResponse::SetActiveTool { tool_name: tool.to_string() }.into());
				self.update_actions();
			}
			SelectPrimaryColor(color) => self.tool_state.document_tool_data.primary_color = *color,
			SelectSecondaryColor(color) => self.tool_state.document_tool_data.secondary_color = *color,
			LogInfo => {
				log::set_max_level(log::LevelFilter::Info);
				log::info!("set log verbosity to info");
			}
			LogDebug => {
				log::set_max_level(log::LevelFilter::Debug);
				log::info!("set log verbosity to debug");
			}
			LogTrace => {
				log::set_max_level(log::LevelFilter::Trace);
				log::info!("set log verbosity to trace");
			}
			_ => consumed = false,
		}

		// pass action to the next level if it was not consumed
		if !consumed {
			let doc = &mut self.documents[self.active_document];
			let tool = self.tool_state.tool_data.active_tool_mut().unwrap().as_mut();
			let document_tool_data = &self.tool_state.document_tool_data;
			consumed = doc.handler.process_action((&mut doc.document, tool, document_tool_data), &input, action, responses, operations)
		}

		// post process action if it was not consumed
		if !consumed {}

		consumed
	}
	fn actions(&self) -> ActionList {
		self.actions.as_slice()
	}
}
