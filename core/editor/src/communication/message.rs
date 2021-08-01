use crate::message_prelude::*;
use graphite_proc_macros::*;
use std::hash::{Hash, Hasher};

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
#[derive(Clone, Debug)]
pub enum Message {
	NoOp,
	#[child]
	Documents(DocumentsMessage),
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

impl Hash for Message {
	fn hash<H>(&self, state: &mut H)
	where
		H: Hasher,
	{
		unsafe { std::mem::transmute::<&Message, &[u8; std::mem::size_of::<Message>()]>(self) }.hash(state);
	}
}

impl PartialEq for Message {
	fn eq(&self, other: &Message) -> bool {
		// TODO: Replace with let [s, o] = [self, other].map(|x| unsafe { std::mem::transmute::<&Message, &[u8; std::mem::size_of::<Message>()]>(x) });
		let vals: Vec<_> = [self, other]
			.iter()
			.map(|x| unsafe { std::mem::transmute::<&Message, &[u8; std::mem::size_of::<Message>()]>(x) })
			.collect();
		vals[0] == vals[1]
	}
}
