use crate::messages::portfolio::document::utility_types::network_interface::InputConnector;
use crate::messages::portfolio::fonts::utility_types::{FontCache, FontCatalog};
use crate::messages::prelude::*;
use graph_craft::application_io::resource::{DataSource, Resource, ResourceHash, ResourceId};
use graph_craft::document::NodeInput;
use graph_craft::document::value::TaggedValue;
use graphene_std::text::Font;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Index of the hidden font-resource input on the text node (after the primary `()` and the text string).
const TEXT_FONT_INPUT_INDEX: usize = 2;

#[derive(ExtractField)]
pub struct FontsMessageContext<'a> {
	pub active_document: &'a DocumentMessageHandler,
}

#[derive(Debug, Default, ExtractField)]
pub struct FontsMessageHandler {
	pub font_catalog: FontCatalog,
	font_to_hash: HashMap<Font, Resource>,
}

#[message_handler_data]
impl MessageHandler<FontsMessage, FontsMessageContext<'_>> for FontsMessageHandler {
	fn process_message(&mut self, message: FontsMessage, responses: &mut VecDeque<Message>, context: FontsMessageContext) {
		let FontsMessageContext { active_document } = context;

		match message {
			FontsMessage::ResourceResolved { family, style, hash } => {
				self.font_to_hash.insert(Font::new(family, style), hash.clone());
			}
			FontsMessage::Load { family, style, response } => {
				if let Some(hash) = self.font_to_hash.get(&Font::new(family.clone(), style.clone())) {
					responses.add(FontsMessage::ResourceResolved { family, style, hash: hash.clone() });
				} else if let Some(url) = self.font_catalog.cached_url(&family, &style) {
					responses.add(FrontendMessage::TriggerResolveResource {
						document_id: active_document.document_id(),
						resource_id: ResourceId::new(),
						url,
					});
				} else if let Some(response) = response {
					responses.add(response);
				}
			}
		}
	}

	advertise_actions!(FontsMessageDiscriminant;
	);
}

impl FontsMessageHandler {}

/// Editor-side cache of loaded font bytes, keyed by [`Font`] and content-addressed by [`ResourceHash`].
/// Used for in-editor text measurement and the editable-textbox overlay (the node graph loads fonts through the resource system instead).
#[derive(Clone, Default)]
pub struct FontCache {
	fonts: HashMap<Font, FontCacheEntry>,
}

#[derive(Clone)]
struct FontCacheEntry {
	hash: ResourceHash,
	data: Resource,
}

impl std::fmt::Debug for FontCache {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FontCache").field("fonts", &self.fonts.keys().collect::<Vec<_>>()).finish()
	}
}

impl FontCache {
	/// Loaded font bytes as a [`Blob`], or `None` if the font has not been loaded yet.
	pub fn get_blob(&self, font: &Font) -> Option<Blob<u8>> {
		self.fonts.get(font).map(|entry| Blob::new((&entry.data).into()))
	}

	pub fn get_resource(&self, font: &Font) -> Option<Resource> {
		self.fonts.get(font).map(|entry| entry.data.clone())
	}

	pub fn hash(&self, font: &Font) -> Option<ResourceHash> {
		self.fonts.get(font).map(|entry| entry.hash)
	}

	pub fn contains(&self, font: &Font) -> bool {
		self.fonts.contains_key(font)
	}

	pub fn insert(&mut self, font: Font, hash: ResourceHash, data: Resource) {
		self.fonts.insert(font, FontCacheEntry { hash, data });
	}

	pub fn used_hashes(&self) -> impl Iterator<Item = ResourceHash> + '_ {
		self.fonts.values().map(|entry| entry.hash)
	}
}
