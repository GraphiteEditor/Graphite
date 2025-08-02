use cef::{CefString, Frame, ImplBinaryValue, ImplBrowser, ImplFrame, ImplListValue, ImplProcessMessage, ImplV8Context, ProcessId, V8Context, sys::cef_process_id_t};

use super::{Context, Initialized};

pub(crate) enum MessageType {
	SendToJS,
	SendToNative,
}
impl From<MessageType> for MessageInfo {
	fn from(val: MessageType) -> Self {
		match val {
			MessageType::SendToJS => MessageInfo {
				name: "send_to_js".to_string(),
				target: cef_process_id_t::PID_RENDERER.into(),
			},
			MessageType::SendToNative => MessageInfo {
				name: "send_to_native".to_string(),
				target: cef_process_id_t::PID_BROWSER.into(),
			},
		}
	}
}
impl TryFrom<String> for MessageType {
	type Error = ();
	fn try_from(value: String) -> Result<Self, Self::Error> {
		match value.as_str() {
			"send_to_js" => Ok(MessageType::SendToJS),
			"send_to_native" => Ok(MessageType::SendToNative),
			_ => Err(()),
		}
	}
}

pub(crate) struct MessageInfo {
	name: String,
	target: ProcessId,
}

pub(crate) trait SendMessage {
	fn send_message(&self, message_type: MessageType, message: &[u8]);
}
impl SendMessage for Context<Initialized> {
	fn send_message(&self, message_type: MessageType, message: &[u8]) {
		let Some(browser) = &self.browser else {
			tracing::error!("Browser is not initialized, cannot send message");
			return;
		};

		let Some(frame) = browser.main_frame() else {
			tracing::error!("Main frame is not available, cannot send message");
			return;
		};

		frame.send_message(message_type, message);
	}
}
impl SendMessage for Option<V8Context> {
	fn send_message(&self, message_type: MessageType, message: &[u8]) {
		let Some(context) = self else {
			tracing::error!("Current V8 context is not available, cannot send message");
			return;
		};

		context.send_message(message_type, message);
	}
}
impl SendMessage for V8Context {
	fn send_message(&self, message_type: MessageType, message: &[u8]) {
		let Some(frame) = self.frame() else {
			tracing::error!("Current V8 context does not have a frame, cannot send message");
			return;
		};

		frame.send_message(message_type, message);
	}
}
impl SendMessage for Frame {
	fn send_message(&self, message_type: MessageType, message: &[u8]) {
		let MessageInfo { name, target } = message_type.into();

		let Some(mut process_message) = cef::process_message_create(Some(&CefString::from(name.as_str()))) else {
			tracing::error!("Failed to create process message: {}", name);
			return;
		};
		let Some(arg_list) = process_message.argument_list() else { return };
		let mut value = ::cef::binary_value_create(Some(message));
		arg_list.set_binary(0, value.as_mut());

		self.send_process_message(target, Some(&mut process_message));
	}
}

pub(crate) struct UnpackedMessage<'a> {
	pub(crate) message_type: MessageType,
	pub(crate) data: &'a [u8],
}

trait Sealed {}
impl Sealed for cef::ProcessMessage {}
#[allow(private_bounds)]
pub(crate) trait UnpackMessage: Sealed {
	/// # Safety
	///
	/// The caller must ensure that the message is valid.
	/// Message should come from cef.
	unsafe fn unpack(&self) -> Option<UnpackedMessage<'_>>;
}
impl UnpackMessage for cef::ProcessMessage {
	unsafe fn unpack(&self) -> Option<UnpackedMessage<'_>> {
		let pointer: *mut cef::sys::_cef_string_utf16_t = self.name().into();
		let message = unsafe { super::utility::pointer_to_string(pointer) };
		let Ok(message_type) = message.try_into() else {
			tracing::error!("Failed to get message type from process message");
			return None;
		};
		let arglist = self.argument_list()?;
		let binary = arglist.binary(0)?;
		let size = binary.size();
		let ptr = binary.raw_data();
		let buffer = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) };
		Some(UnpackedMessage { message_type, data: buffer })
	}
}
