use crate::messages::portfolio::fonts::utility_types::FontCatalog;
use crate::messages::prelude::*;
use graph_craft::application_io::resource::{Resource, ResourceHash};
use graphene_std::text::Font;

#[impl_message(Message, PortfolioMessage, Fonts)]
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum FontsMessage {
	CatalogLoaded {
		catalog: FontCatalog,
	},
	ResourceResolved {
		font: Font,
		hash: ResourceHash,
	},
	Load {
		font: Font,
		response: Box<Message>,
	},
	Cached {
		hash: ResourceHash,
		#[serde(skip, default = "Resource::empty")]
		resource: Resource,
	},
}
