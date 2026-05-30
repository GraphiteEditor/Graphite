use crate::messages::portfolio::utility_types::FontCatalog;
use crate::messages::prelude::*;
use graph_craft::application_io::resource::ResourceId;
use graph_craft::document::NodeId;
use graphene_std::text::Font;

#[impl_message(Message, PortfolioMessage, Fonts)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum FontsMessage {
	ResourceResolved { family: String, style: String, hash: ResourceHash },
	Load { family: String, style: String, response: Option<Message> },
}
