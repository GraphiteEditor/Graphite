pub use crate::derivable_custom_traits::{ToDiscriminant, TransitiveChild};
use graphite_proc_macros::impl_message;
use graphite_proc_macros::*;
pub use prelude::*;

pub trait AsMessage: TransitiveChild
where
	Self::TopParent: TransitiveChild<Parent = Self::TopParent, TopParent = Self::TopParent> + AsMessage,
{
	fn local_name(self) -> String;
	fn global_name(self) -> String {
		<Self as Into<Self::TopParent>>::into(self).local_name()
	}
}

#[impl_message]
#[derive(PartialEq, Clone, Debug)]
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
	pub use super::{AsMessage, Message, MessageDiscriminant, ToDiscriminant, TransitiveChild};
	pub use graphite_proc_macros::*;
}
