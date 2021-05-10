use document_core::DocumentResponse;

use super::{Action, ActionHandler, InputPreprocessor, Operation, Response};
use crate::SvgDocument;
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

		// process action before passing them further down
		use Action::*;
		match action {
			DeleteLayer(path) => operations.push(Operation::DeleteLayer { path: path.clone() }),
			_ => consumed = false,
		}

		// pass action to the next level if it was not consumed
		if !consumed {
			let (doc, tool, data) = data;
			consumed = tool.process_action((doc, data), &input, action, responses, operations)
		}

		// post process action if it was not consumed
		if !consumed {
			// Ctrl + Z
		}

		consumed
	}
	fn actions(&self) -> &[(&str, Action)] {
		&[("", Action::Save)]
	}
}

impl DocumentActionHandler {
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
