use crate::message_prelude::*;

use graphite_proc_macros::*;

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
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

#[remain::sorted]
#[impl_message]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Message {
	#[remain::unsorted]
	NoOp,
	#[child]
	Frontend(FrontendMessage),
	#[child]
	Global(GlobalMessage),
	#[child]
	InputMapper(InputMapperMessage),
	#[child]
	InputPreprocessor(InputPreprocessorMessage),
	#[child]
	Layout(LayoutMessage),
	#[child]
	Portfolio(PortfolioMessage),
	#[child]
	Tool(ToolMessage),
}

impl Message {
	/// Returns the byte representation of the message.
	///
	/// # Safety
	/// This function reads from uninitialized memory!!!
	/// Only use if you know what you are doing.
	unsafe fn as_slice(&self) -> &[u8] {
		core::slice::from_raw_parts(self as *const Message as *const u8, std::mem::size_of::<Message>())
	}

	/// Returns a pseudo hash that should uniquely identify the message.
	/// This is needed because `Hash` is not implemented for `f64`s
	///
	/// # Safety
	/// This function reads from uninitialized memory but the generated value should be fine.
	pub fn pseudo_hash(&self) -> u64 {
		let mut s = DefaultHasher::new();
		unsafe { self.as_slice() }.hash(&mut s);
		s.finish()
	}
}
