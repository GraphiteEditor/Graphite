use crate::consts::{FONT_CATALOG_REQUEST_TIMEOUT_MILLISECONDS, FONT_CATALOG_RETRY_DELAY_MILLISECONDS};
use crate::messages::portfolio::fonts::FALLBACK_FONT_RESOURCE;
use crate::messages::portfolio::fonts::utility_types::FontCatalog;
use crate::messages::prelude::*;
use graph_craft::application_io::resource::{DataSource, Resource, ResourceHash, ResourceId};
use graphene_std::text::Font;
use std::sync::Arc;

#[derive(ExtractField)]
pub struct FontsMessageContext<'a> {
	pub resource_storage: &'a ResourceStorageMessageHandler,
	pub current_time: u64,
}

#[derive(Debug, Default, ExtractField)]
pub struct FontsMessageHandler {
	pub font_catalog: Arc<FontCatalog>,
	catalog_request: CatalogRequest,
	font_hashes: HashMap<Font, ResourceHash>,
	font_data: HashMap<ResourceHash, Resource>,
}

/// State of the font catalog request shared by every `FontsMessage::LoadCatalog`.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum CatalogRequest {
	#[default]
	Idle,
	/// A request is unanswered or the last one failed, so another waits until this time.
	Waiting { until: u64 },
}

#[message_handler_data]
impl MessageHandler<FontsMessage, FontsMessageContext<'_>> for FontsMessageHandler {
	fn process_message(&mut self, message: FontsMessage, responses: &mut VecDeque<Message>, context: FontsMessageContext) {
		let FontsMessageContext { resource_storage, current_time } = context;

		match message {
			FontsMessage::LoadCatalog => {
				// Senders ask on every message until the catalog arrives, so wait out an unanswered or failed request before sending another
				let waiting = matches!(self.catalog_request, CatalogRequest::Waiting { until } if current_time < until);
				if !self.font_catalog.is_empty() || waiting {
					return;
				}
				self.catalog_request = CatalogRequest::Waiting {
					until: current_time + FONT_CATALOG_REQUEST_TIMEOUT_MILLISECONDS,
				};

				responses.add(NetworkMessage::request(async move |client| {
					let Some(catalog) = FontCatalog::load_from_api(&client).await else {
						log::error!("Failed to load the font catalog");
						return FontsMessage::CatalogLoadFailed.into();
					};
					FontsMessage::CatalogLoaded { catalog }.into()
				}));
			}
			FontsMessage::CatalogLoadFailed => {
				self.catalog_request = CatalogRequest::Waiting {
					until: current_time + FONT_CATALOG_RETRY_DELAY_MILLISECONDS,
				};
			}
			FontsMessage::CatalogLoaded { catalog } => {
				// An empty catalog counts as a failure, and one equal to the current catalog (from overlapping requests) changes nothing
				if catalog.is_empty() {
					self.catalog_request = CatalogRequest::Waiting {
						until: current_time + FONT_CATALOG_RETRY_DELAY_MILLISECONDS,
					};
					return;
				}
				self.catalog_request = CatalogRequest::Idle;
				if *self.font_catalog == catalog {
					return;
				}

				self.font_catalog = Arc::new(catalog);
				responses.add(PortfolioMessage::ResolveResources);
				responses.add(ToolMessage::RefreshToolOptions);
				responses.add(PropertiesPanelMessage::Refresh);
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
		self.font_catalog.normalize(font)
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::messages::portfolio::fonts::utility_types::FontCatalogFamily;

	fn process(handler: &mut FontsMessageHandler, message: FontsMessage, current_time: u64) -> VecDeque<Message> {
		let resource_storage = ResourceStorageMessageHandler::default();
		let mut responses = VecDeque::new();
		handler.process_message(
			message,
			&mut responses,
			FontsMessageContext {
				resource_storage: &resource_storage,
				current_time,
			},
		);
		responses
	}

	fn catalog_requests_sent(handler: &mut FontsMessageHandler, current_time: u64) -> usize {
		let responses = process(handler, FontsMessage::LoadCatalog, current_time);
		responses.iter().filter(|message| matches!(message, Message::Network(_))).count()
	}

	fn catalog() -> FontCatalog {
		FontCatalog::from(vec![FontCatalogFamily {
			name: "Lato".to_string(),
			styles: Vec::new(),
		}])
	}

	#[test]
	fn catalog_requests_wait_for_an_unanswered_or_failed_one() {
		let mut handler = FontsMessageHandler::default();

		assert_eq!(catalog_requests_sent(&mut handler, 0), 1);
		assert_eq!(
			catalog_requests_sent(&mut handler, FONT_CATALOG_REQUEST_TIMEOUT_MILLISECONDS - 1),
			0,
			"An unanswered request should block another"
		);
		assert_eq!(
			catalog_requests_sent(&mut handler, FONT_CATALOG_REQUEST_TIMEOUT_MILLISECONDS),
			1,
			"A request unanswered for too long should be retried"
		);

		// A failure, or an empty catalog in its place, waits the retry delay
		for (failure_time, failure) in [(100_000, FontsMessage::CatalogLoadFailed), (200_000, FontsMessage::CatalogLoaded { catalog: FontCatalog::default() })] {
			assert!(process(&mut handler, failure, failure_time).is_empty());

			let retry_time = failure_time + FONT_CATALOG_RETRY_DELAY_MILLISECONDS;
			assert_eq!(
				catalog_requests_sent(&mut handler, retry_time - 1),
				0,
				"A failure should block another request until the retry delay passes"
			);
			assert_eq!(catalog_requests_sent(&mut handler, retry_time), 1);
		}
	}

	#[test]
	fn catalog_arrivals_after_the_first_change_nothing() {
		let mut handler = FontsMessageHandler::default();

		assert!(!process(&mut handler, FontsMessage::CatalogLoaded { catalog: catalog() }, 0).is_empty());
		assert_eq!(*handler.font_catalog, catalog());
		assert_eq!(catalog_requests_sent(&mut handler, 0), 0, "A loaded catalog should need no request");

		assert!(
			process(&mut handler, FontsMessage::CatalogLoaded { catalog: catalog() }, 0).is_empty(),
			"An equal catalog should send no refreshes"
		);
		assert!(process(&mut handler, FontsMessage::CatalogLoaded { catalog: FontCatalog::default() }, 0).is_empty());
		assert_eq!(*handler.font_catalog, catalog(), "An empty catalog should not replace a loaded one");
	}
}
