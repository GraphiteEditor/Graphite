pub mod events;
use crate::EditorError;
use events::{Event, Response};

pub type Callback = Box<dyn Fn(Response)>;
pub struct Dispatcher {
	callback: Callback,
}

impl Dispatcher {
	pub fn handle_event(&self, event: Event) -> Result<(), EditorError> {
		match event {
			Event::Click(_) => Ok(self.emit_response(Response::UpdateCanvas)),
			_ => todo!(),
		}
	}
	pub fn emit_response(&self, response: Response) {
		let func = &self.callback;
		func(response)
	}
	pub fn new(callback: Callback) -> Dispatcher {
		Dispatcher { callback }
	}
}
