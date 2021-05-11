use document_core::DocumentResponse;

use super::{Action, ActionHandler, InputPreprocessor, Operation, Response};
use crate::{events::ToolResponse, SvgDocument};
use crate::{
	tools::{DocumentToolData, ToolActionHandlerData},
	EditorError,
};

#[derive(Debug, Default, Clone)]
pub struct DocumentActionHandler {}

impl ActionHandler<(&mut SvgDocument, &mut dyn for<'a> ActionHandler<ToolActionHandlerData<'a>>, &DocumentToolData)> for DocumentActionHandler {
	fn process_action(
		&mut self,
		data: (&mut SvgDocument, &mut dyn for<'a> ActionHandler<ToolActionHandlerData<'a>>, &DocumentToolData),
		input: &InputPreprocessor,
		action: &Action,
		responses: &mut Vec<Response>,
		operations: &mut Vec<Operation>,
	) -> bool {
		let mut consumed = true;
		let (doc, tool, data) = data;

		// process action before passing them further down
		use Action::*;
		match action {
			DeleteLayer(path) => operations.push(Operation::DeleteLayer { path: path.clone() }),
			AddFolder(path) => operations.push(Operation::AddFolder { path: path.clone() }),
			_ => consumed = false,
		}

		// pass action to the next level if it was not consumed
		if !consumed {
			consumed = tool.process_action((doc, data), &input, action, responses, operations)
		}

		// post process action if it was not consumed
		if !consumed {
			consumed = true;
			match action {
				Undo => {
					// this is a temporary fix and will be addressed by #123
					if let Some(id) = doc.root.list_layers().last() {
						operations.push(Operation::DeleteLayer { path: vec![*id] })
					}
				}
				_ => consumed = false,
			}
		}

		let mut document_responses = self.dispatch_operations(doc, operations.drain(..));
		let canvas_dirty = self.filter_document_responses(&mut document_responses);
		responses.extend(document_responses.drain(..).map(Into::into));
		if canvas_dirty {
			responses.push(ToolResponse::UpdateCanvas { document: doc.render_root() }.into())
		}

		consumed
	}
	actions_fn!(Action::Undo, Action::DeleteLayer(vec![]), Action::AddFolder(vec![]));
}

impl DocumentActionHandler {
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

	fn dispatch_operations<I: IntoIterator<Item = Operation>>(&self, document: &mut SvgDocument, operations: I) -> Vec<DocumentResponse> {
		let mut responses = vec![];
		for operation in operations {
			match self.dispatch_operation(document, operation) {
				Ok(Some(mut res)) => {
					responses.append(&mut res);
				}
				Ok(None) => (),
				Err(error) => log::error!("{}", error),
			}
		}
		responses
	}

	fn dispatch_operation(&self, document: &mut SvgDocument, operation: Operation) -> Result<Option<Vec<DocumentResponse>>, EditorError> {
		Ok(document.handle_operation(operation)?)
	}
}
