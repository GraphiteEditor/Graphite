use std::collections::HashMap;
use std::thread;
use std::thread::JoinHandle;

use crate::error::ServerError;
use crate::group::{Message, SendGroup};
use graphite_editor_core::message_prelude::*;
use graphite_editor_core::Editor;
use log::{error, info};

pub trait Game {
	fn run(self) -> Result<JoinHandle<()>, ServerError>;
}

#[allow(dead_code)]
pub struct Graphite {
	game: (),
	group: SendGroup,
	users: Vec<User>,
	will_to_live: bool,
	res_cache: HashMap<u32, Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct User {
	name: String,
	sender: ws::Sender,
}

impl User {
	pub fn new(name: String, sender: ws::Sender) -> Self {
		User { name, sender }
	}
}

impl Game for Graphite {
	fn run(self) -> Result<JoinHandle<()>, ServerError> {
		thread::Builder::new()
			.name(format!("group{}", self.group.id))
			.spawn(move || self.game_loop())
			.map_err(ServerError::GameCreation)
	}
}

impl Graphite {
	pub fn new(group: SendGroup) -> Self {
		Self {
			game: (),
			group,
			users: Vec::new(),
			will_to_live: true,
			res_cache: HashMap::new(),
		}
	}

	fn game_loop(mut self) {
		let mut editor = Editor::new();
		while self.will_to_live {
			let messages = self.get_messages();
			let mut responses = Vec::new();
			for message in messages.into_iter() {
				if let Message::Data((ip, data)) = message {
					let message: graphite_editor_core::message_prelude::Message = serde_json::from_str(&String::from_utf8(data).unwrap()).unwrap();
					responses.extend(editor.handle_message(message));
				}
			}
			//game.handle_events(messages);
			//game.tick();
			//let b = game.get_broadcast()
			//self.users.iter().foreach(|u| u.sender.send(b));
			let messages = self.get_messages();
			//thread::sleep(std::time::Duration::from_secs(5));
		}
		info!("thread killed itself");
	}

	fn add_user(&mut self, user: &User) {
		self.users.push(user.clone());
	}

	fn get_messages(&mut self) -> Vec<Message> {
		//  info!("receiver {:#?} is still alive", self.group.receiver);
		let (mut data, control): (Vec<Message>, Vec<Message>) = self.group.receiver.try_iter().partition(Message::is_data);
		control.iter().for_each(|x| match x {
			Message::Park => {
				data = Vec::new();
				thread::park();
			}
			Message::Kill => self.will_to_live = false,
			Message::Add(user) => self.add_user(&user),
			Message::Remove(sender) => {
				if let Some(pos) = self.users.iter().position(|x| x.sender == *sender) {
					self.users.swap_remove(pos);
				}
			}
			_ => (),
		});
		data
	}
}
