use document_core::DocumentResponse;

use crate::{document::Document, events::ToolResponse, tools::ToolType, Color, SvgDocument};

use super::{document_action_handler::DocumentMessage, tool_action_handler::ToolMessage};
use super::{input_manager::InputPreprocessor, Action, ActionHandler, ActionList, Response};
use crate::tools::ToolFsmState;

use strum_macros::{AsRefStr, Display, EnumDiscriminants, EnumIter, EnumString};

#[derive(Debug, Clone, Display, AsRefStr, EnumDiscriminants, EnumIter, EnumString)]
pub enum GlobalAction {
	Document(DocumentMessage),
	Tool(ToolMessage),
	LogInfo,
	LogDebug,
	LogTrace,
	SelectDocument(usize),
}

#[derive(Debug)]
pub struct GlobalActionHandler {
	documents: Vec<Document>,
	active_document: usize,
	tool_state: ToolFsmState,
	actions: Vec<&'static [&'static str]>,
}

impl GlobalActionHandler {
	pub fn new() -> Self {
		Self {
			documents: vec![Document::default()],
			active_document: 0,
			tool_state: ToolFsmState::default(),
			actions: Vec::new(),
		}
	}
	fn current_actions<'a>() -> dyn FnOnce(&Self) -> ActionList<'a> {
		/*use Action::*;
		actions!(
			SelectDocument(0),
			SelectTool(ToolType::Select),
			LogInfo,
			LogDebug,
			LogTrace,
			SelectPrimaryColor(Color::WHITE),
			SelectSecondaryColor(Color::BLACK)
		)*/
		|| &[&[]]
	}
	fn update_actions(&mut self) {
		self.actions.clear();
		self.actions.extend(Self::current_actions()(&self));
		if let Ok(tool) = self.tool_state.tool_data.active_tool() {
			self.actions.extend(tool.actions());
		}
		let document = &self.documents[self.active_document];
		self.actions.extend(document.handler.actions());
	}

	fn filter_document_responses(&self, document_responses: &mut Vec<DocumentResponse>) -> bool {
		//let changes = document_responses.drain_filter(|x| x == DocumentResponse::DocumentChanged);
		let mut canvas_dirty = false;
		let mut i = 0;
		while i < document_responses.len() {
			if matches!(document_responses[i], DocumentResponse::DocumentChanged) {
				canvas_dirty = true;
				document_responses.remove(i);
			} else {
				i += 1;
			}
		}
		canvas_dirty
	}
}

impl ActionHandler<GlobalAction, &InputPreprocessor> for GlobalActionHandler {
	fn process_action(&mut self, action: GlobalAction, data: &InputPreprocessor, responses: &mut Vec<Response>) {
		let mut consumed = true;

		// process action before passing them further down
		/*use Action::*;
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
		*/
	}
	fn actions(&self) -> ActionList {
		self.actions.as_slice()
	}
}
