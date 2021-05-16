use proc_macros::MessageImpl;
use std::fmt::Display;

use prelude::*;

pub trait AsMessage: Sized + Into<Message> + Send + Sync + PartialEq<Message> + Display + Clone {
	fn name(&self) -> String;
	fn suffix(&self) -> &'static str;
	fn prefix() -> String;
	fn get_discriminant(&self) -> MessageDiscriminant;
}

#[derive(MessageImpl, PartialEq, Clone)]
#[message(Message, Message, Child)]
pub enum Message {
	NoOp,
	#[child]
	Document(DocumentMessage),
	#[child]
	Global(GlobalMessage),
	#[child]
	Tool(ToolMessage),
	#[child]
	Frontend(FrontendMessage),
	#[child]
	InputPreprocessor(InputPreprocessorMessage),
	#[child]
	InputMapper(InputMapperMessage),
}

pub mod prelude {
	pub use super::super::{
		super::tools::rectangle::{RectangleMessage, RectangleMessageDiscriminant},
		document_action_handler::{DocumentMessage, DocumentMessageDiscriminant},
		frontend::{FrontendMessage, FrontendMessageDiscriminant},
		global_action_handler::{GlobalMessage, GlobalMessageDiscriminant},
		input_manager::{InputMapperMessage, InputMapperMessageDiscriminant, InputPreprocessorMessage, InputPreprocessorMessageDiscriminant},
		tool_action_handler::{ToolMessage, ToolMessageDiscriminant},
	};
}
