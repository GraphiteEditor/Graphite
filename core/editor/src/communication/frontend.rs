use super::{AsMessage, Message, MessageDiscriminant};
use proc_macros::MessageImpl;

#[derive(MessageImpl, PartialEq, Clone)]
#[message(Message, Message, Frontend)]
pub enum FrontendMessage {
	Foo,
}
