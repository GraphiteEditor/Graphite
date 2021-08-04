use crate::message_prelude::*;
use graphite_proc_macros::*;
use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
};

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
#[derive(Clone, Debug, PartialEq)]
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

impl Message {
	unsafe fn as_slice(&self) -> &[u8] {
		core::slice::from_raw_parts(self as *const Message as *const u8, std::mem::size_of::<Message>())
	}
	pub fn pseudo_hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		unsafe { self.as_slice() }.hash(&mut s);
		s.finish()
	}
}
