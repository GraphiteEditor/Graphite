use std::sync::mpsc;
use std::thread::JoinHandle;

use crate::error::ServerError;
use crate::games;
use crate::games::{Game, Graphite};
use log::info;
use ws::Sender;

pub type GroupId = String;

#[derive(Debug)]
/// capacity is never allowed to be above usize::MAX
pub struct Group {
	pub clients: Vec<Sender>,
	pub sender: mpsc::Sender<Message>,
	id: GroupId,
	game_thread: JoinHandle<()>,
}

pub struct SendGroup {
	pub receiver: mpsc::Receiver<Message>,
	pub id: GroupId,
}

#[derive(Debug)]
pub enum Message {
	// TODO: flatten tuple
	Data((String, Vec<u8>)),
	Park,
	Kill,
	Add(games::User),
	Remove(Sender),
}

impl Message {
	pub fn is_data(&self) -> bool {
		match self {
			Message::Data(_) => true,
			_ => false,
		}
	}
}

impl Drop for Group {
	fn drop(&mut self) {
		info!("dropping group {:?}", &self);
		let _ = self.sender.send(Message::Kill);
	}
}

impl Group {
	pub fn id(&self) -> GroupId {
		self.id.clone()
	}

	#[allow(dead_code)]
	pub fn park(self) -> Result<(), ServerError> {
		Ok(self.sender.send(Message::Park)?)
	}

	#[allow(dead_code)]
	pub fn unpark(&mut self) {
		self.game_thread.thread().unpark();
	}

	pub fn add_client(&mut self, client: Sender) -> Result<mpsc::Sender<Message>, ServerError> {
		self.clients.push(client.clone());
		self.sender
			.send(Message::Add(games::User::new("None".to_owned(), client)))
			.map_err(Into::into)
			.map(|()| self.sender.clone())
	}

	pub fn remove_client(&mut self, client: &Sender) -> Result<(), ServerError> {
		if let Some(pos) = self.clients.iter().position(|x| *x == *client) {
			self.clients.swap_remove(pos);
		}
		self.sender.send(Message::Remove(client.clone())).map_err(Into::into)
	}

	pub fn new(name: String) -> Result<Self, ServerError> {
		let (sender, receiver) = mpsc::channel();
		info!("Creating Group {}", name);

		let send_group = SendGroup { receiver, id: name.clone() };

		let game = Graphite::new(send_group);

		info!("Creating Group {}", name);
		Ok(Self {
			clients: Vec::new(),
			sender,
			id: name,
			game_thread: game.run()?,
		})
	}
}
