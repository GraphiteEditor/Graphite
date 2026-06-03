use crate::messages::portfolio::fonts::FALLBACK_FONT_RESOURCE;
use crate::messages::portfolio::fonts::utility_types::FontCatalog;
use crate::messages::prelude::*;
use graph_craft::application_io::resource::{DataSource, Resource, ResourceHash, ResourceId};
use graphene_std::text::Font;

#[derive(ExtractField)]
pub struct FontsMessageContext<'a> {
	pub resource_storage: &'a ResourceStorageMessageHandler,
}

#[derive(Debug, Default, ExtractField)]
pub struct FontsMessageHandler {
	pub font_catalog: FontCatalog,
	font_hashes: HashMap<Font, ResourceHash>,
	font_data: HashMap<ResourceHash, Resource>,
}

#[message_handler_data]
impl MessageHandler<FontsMessage, FontsMessageContext<'_>> for FontsMessageHandler {
	fn process_message(&mut self, message: FontsMessage, responses: &mut VecDeque<Message>, context: FontsMessageContext) {
		let FontsMessageContext { resource_storage } = context;

		match message {
			FontsMessage::CatalogLoaded { catalog } => {
				self.font_catalog = catalog;
				responses.add(PortfolioMessage::ResolveResources);
			}
			FontsMessage::ResourceResolved { font, hash } => {
				let font = self.normalize(font);
				self.font_hashes.insert(font, hash);
			}
			FontsMessage::Load { font, response } => {
				let font = self.normalize(font);
				let Some(hash) = self.font_hashes.get(&font).copied() else {
					log::warn!("FontsMessage::Load for {font:?} with no known hash; ignoring");
					return;
				};
				if self.font_data.contains_key(&hash) {
					responses.add(*response);
					return;
				}
				let loader = resource_storage.resources();
				responses.add(async move {
					let resource = loader.load(hash).await;
					match resource {
						Some(resource) => Message::Batched {
							messages: Box::new([FontsMessage::Cached { hash, resource }.into(), *response]),
						},
						None => {
							log::warn!("Storage missing data for font hash {hash}");
							*response
						}
					}
				});
			}
			FontsMessage::Cached { hash, resource } => {
				self.font_data.insert(hash, resource);
			}
		}
	}

	advertise_actions!(FontsMessageDiscriminant;);
}

impl FontsMessageHandler {
	pub fn cached_hash(&self, font: &Font) -> Option<ResourceHash> {
		let font = self.normalize(font.clone());
		self.font_hashes.get(&font).copied()
	}

	pub fn cached_url(&self, font: &Font) -> Option<String> {
		let font = self.normalize(font.clone());
		self.font_catalog.download_url(&font)
	}

	pub fn get_resource_or_queue_load(&self, font: &Font, responses: &mut VecDeque<Message>) -> Resource {
		let font = self.normalize(font.clone());
		if let Some(hash) = self.font_hashes.get(&font) {
			if let Some(resource) = self.font_data.get(hash) {
				return resource.clone();
			}
			responses.add(FontsMessage::Load {
				font: font.clone(),
				response: Message::NoOp.into(),
			});
		}
		FALLBACK_FONT_RESOURCE.clone()
	}

	pub fn id_font(&self, resources: &ResourceMessageHandler, resource_id: ResourceId) -> Option<Font> {
		let info = resources.registry.info(&resource_id)?;
		info.sources.iter().find_map(|source| match source {
			DataSource::Font { family, style } => Some(match style {
				Some(style) => Font::new(family.clone(), style.clone()),
				None => Font::new_with_default_style(family.clone()),
			}),
			_ => None,
		})
	}

	pub fn used_resources(&self) -> impl Iterator<Item = ResourceHash> + '_ {
		self.font_hashes.values().copied().chain(self.font_data.keys().copied())
	}

	fn normalize(&self, font: Font) -> Font {
		match self.font_catalog.find_font_style_in_catalog(&font) {
			Some(style) => Font::new(font.font_family, style.to_named_style()),
			None => font,
		}
	}
}
