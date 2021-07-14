use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

use crate::error::ServerError;
use crate::group::{Group, GroupId, Message as GroupMessage};
use log::{error, info, warn};
use ws::{listen, CloseCode, Handler, Handshake, Message, Request, Response, Sender};

const PROTOCOL: &str = "tuesday";
// WebSocket connection handler for the server connection
pub struct Socket {
	ws: Sender,
	group: mpsc::Sender<GroupMessage>,
	groups: Arc<Mutex<HashMap<String, Group>>>,
	ip: String,
	id: GroupId,
}

pub fn run(address: &str, port: &str) -> Result<JoinHandle<()>, ServerError> {
	let count = Arc::new(Mutex::new(HashMap::new()));
	let (sender, _) = mpsc::channel();
	let url = format!("{}:{}", address, port);
	thread::Builder::new()
		.name("server".to_owned())
		.spawn(move || {
			listen(url, |out| Socket {
				ws: out,
				group: sender.clone(),
				groups: count.clone(),
				ip: "No ip".to_owned(),
				id: "".to_owned(),
			})
			.unwrap()
		})
		.map_err(ServerError::WebSocketCreation)
}

impl Handler for Socket {
	// called when the socket connection is created
	fn on_open(&mut self, handshake: Handshake) -> ws::Result<()> {
		if let Ok(Some(ip)) = handshake.remote_addr() {
			self.ip = ip;
		}
		Ok(())
	}

	fn on_message(&mut self, msg: Message) -> ws::Result<()> {
		info!("Socket got message '{}'. ", msg);

		self.group.send(GroupMessage::Data((self.ip.clone(), msg.into_data()))).unwrap_or_else(|err| {
			let err = format!("failed to deliver internal message {}", err);
			error!("{}", err);
			self.ws.close_with_reason(CloseCode::Error, err).unwrap_or_else(|e| error!("failed to send message to client {}", e));
		});

		Ok(())
	}

	fn on_close(&mut self, c: CloseCode, s: &str) {
		info!("removing client {:?} {:?}", c, s);
		if let Ok(mut guard) = self.groups.lock() {
			if let Some(group) = guard.get_mut(&self.id) {
				if group.remove_client(&self.ws).is_err() {
					warn!("failed to remove Client from Game");
				}
				if group.clients.is_empty() {
					guard.remove(&self.id); // TODO
				}
			}
		}
	}

	// low-level handling of requests
	fn on_request(&mut self, req: &Request) -> ws::Result<Response> {
		let (res, token) = handshake(req);
		Ok(match token.and_then(move |response| self.handle_token(response)) {
			Ok(_) => res,
			Err(err) => fail_response(res, format!("Client {:?}: {:?}", req.client_addr(), err).as_str()),
		})
	}
}

impl Socket {
	fn handle_token(&mut self, name: String) -> Result<(), ServerError> {
		match self.groups.lock() {
			Ok(mut guard) => {
				if !guard.contains_key(&name) {
					let group = Group::new(name.clone())?;
					self.group = group.sender.clone();
					self.id = group.id();
					guard.insert(name, group);
				}

				// panics if any thread panicked while using the mutex
				let group = &mut self.group;
				guard.get_mut(&self.id).unwrap().add_client(self.ws.clone()).map(|s| *group = s)
			}
			Err(e) => Err(ServerError::Group(format!("cold not add client {:?}  to group {}: {}", self.ws, name, e))),
		}
	}
}

fn handshake(req: &Request) -> (Response, Result<String, ServerError>) {
	let mut res = Response::from_request(req).unwrap();
	if let Ok(protocols) = req.protocols() {
		/*if protocols.iter().any(|pro| pro.contains(PROTOCOL)) {
			res.set_protocol(PROTOCOL)
		} else {
			return (
				fail_response(
					res,
					format!("does not support the {} protocol", PROTOCOL).as_str(),
				),
				Err(ServerError::InvalidProtocol),
			);
		}*/
		let token = protocols.iter().find(|pro| pro.starts_with("Token-"));
		match token {
			Some(token) => {
				let (_, token) = token.split_at(6);
				(res, Ok(token.to_string()))
			}
			None => (
				fail_response(res, "no token in protocols"),
				Err(ServerError::InvalidToken("No Token was passed as a Protocol in the Sec-WebSocket-Protocol Header".to_string())),
			),
		}
	} else {
		(fail_response(res, "failed to retrieve protocols"), Err(ServerError::InvalidProtocol))
	}
}

fn fail_response(mut res: Response, reason: &str) -> Response {
	res.set_status(400);
	res.set_reason(reason);
	warn!("{}", reason);
	res
}
