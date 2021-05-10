use crate::{
	dispatcher::{Action, ActionHandler, InputPreprocessor, Response},
	tools::ToolActionHandlerData,
};
use document_core::Operation;

#[derive(Default)]
pub struct Eyedropper;

impl<'a> ActionHandler<ToolActionHandlerData<'a>> for Eyedropper {
	fn process_action(&mut self, data: ToolActionHandlerData<'a>, input_preprocessor: &InputPreprocessor, action: &Action, responses: &mut Vec<Response>, operations: &mut Vec<Operation>) -> bool {
		todo!("{}::handle_input {:?} {:?} {:?} {:?}", module_path!(), action, data, responses, operations);
	}
	actions!();
}
