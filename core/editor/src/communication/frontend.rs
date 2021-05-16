use super::{AsMessage, Message, MessageDiscriminant};
use proc_macros::MessageImpl;
use serde::{Deserialize, Serialize};

#[derive(MessageImpl, PartialEq, Clone, Deserialize, Serialize)]
#[message(Message, Message, Frontend)]
pub enum FrontendMessage {
	Foo,
}
