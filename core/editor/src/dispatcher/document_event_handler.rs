use super::{Action, ActionHandler, InputPreprocessor, Operation, Response};
use crate::tools::{DocumentToolData, ToolActionHandlerData};
use crate::SvgDocument;

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
			&DeleteLayer(path) => operations.push(Operation::DeleteLayer { path }),
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
