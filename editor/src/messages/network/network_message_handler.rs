use crate::messages::network::utility_types::Client;
use crate::messages::prelude::*;

#[derive(ExtractField)]
pub struct NetworkMessageContext {}

#[derive(Debug, Default, ExtractField)]
pub struct NetworkMessageHandler {
	client: Client,
}
#[message_handler_data]
impl MessageHandler<NetworkMessage, NetworkMessageContext> for NetworkMessageHandler {
	fn process_message(&mut self, message: NetworkMessage, responses: &mut VecDeque<Message>, _context: NetworkMessageContext) {
		match message {
			NetworkMessage::Request { request } => {
				if let Some(request) = request {
					responses.add(request(self.client.clone()));
				} else {
					log::error!("received a empty NetworkMessage::Request");
				}
			}
		}
	}

	advertise_actions!(NetworkMessageDiscriminant;
	);
}
