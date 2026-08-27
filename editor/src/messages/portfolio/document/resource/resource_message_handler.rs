use crate::messages::network::Client;
use crate::messages::portfolio::{document::resource::utility_types::EmbeddedResources, fonts::utility_types::FontCatalog};
use crate::messages::prelude::*;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use graph_craft::application_io::resource::{DataSource, LoadResource, Resource, ResourceHash, ResourceId, ResourceRegistry};
use graphene_std::text::Font;
use std::sync::Arc;
use url::Url;

#[derive(ExtractField)]
pub struct ResourceMessageContext<'a> {
	pub document_id: DocumentId,
	pub fonts: &'a FontsMessageHandler,
}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, ExtractField)]
pub struct ResourceMessageHandler {
	pub registry: ResourceRegistry,
	pub embedded: EmbeddedResources,
	#[serde(skip)]
	pending_resolves: HashSet<ResourceId>,
}

#[message_handler_data]
impl MessageHandler<ResourceMessage, ResourceMessageContext<'_>> for ResourceMessageHandler {
	fn process_message(&mut self, message: ResourceMessage, responses: &mut VecDeque<Message>, context: ResourceMessageContext) {
		let ResourceMessageContext { document_id, fonts } = context;

		match message {
			ResourceMessage::StoreEmbedded { resource_id, data } => {
				let hash = ResourceHash::from(data.as_ref());
				self.registry.push_source_back(&resource_id, DataSource::Embedded);
				self.registry.resolve(&resource_id, hash);
				responses.add(ResourceStorageMessage::Store { data });
			}
			ResourceMessage::AddFont { resource_id, font } => {
				let style = fonts.font_catalog.find_font_style_in_catalog(&font);
				let style_name = style.map(|style| style.to_named_style()).unwrap_or_else(|| font.font_style.clone());
				self.registry.push_source_back(&resource_id, DataSource::Embedded);
				self.registry.push_source_back(
					&resource_id,
					DataSource::Font {
						family: font.font_family,
						style: Some(style_name),
					},
				);
				responses.add(ResourceMessage::Resolve { resource_id });
			}
			ResourceMessage::ResolveAll => {
				let unresolved_ids: Vec<ResourceId> = self.registry.unresolved().map(|info| info.id).collect();
				for id in unresolved_ids {
					if self.pending_resolves.contains(&id) {
						continue;
					}
					responses.add(ResourceMessage::Resolve { resource_id: id });
				}
			}
			ResourceMessage::Resolve { resource_id } => {
				if self.pending_resolves.contains(&resource_id) {
					log::warn!("Already pending resolve for {resource_id}; skipping");
					return;
				}
				let Some(info) = self.registry.info(&resource_id) else {
					log::error!("Resolve for {resource_id}: no registry entry");
					return;
				};
				if info.hash.is_some() {
					log::warn!("Resource {resource_id} already resolved");
					return;
				}

				self.pending_resolves.insert(resource_id);

				let font_catalog = fonts.font_catalog.clone();

				let sources = info
					.sources
					.iter()
					.map(|source| match source {
						DataSource::Font { family, style } => {
							let font = match style {
								Some(style) => Font::new(family.clone(), style.clone()),
								None => Font::new_with_default_style(family.clone()),
							};
							let hash = fonts.cached_hash(&font);
							(source.clone(), hash)
						}
						source => (source.clone(), None),
					})
					.collect::<Vec<(DataSource, Option<ResourceHash>)>>();

				async fn resolve_to_message(document_id: DocumentId, resource_id: ResourceId, source: DataSource, url: Url, client: &Client) -> Option<Message> {
					let result = client.fetch(url.clone()).await;
					match result {
						Some(data) => {
							let hash = ResourceHash::from(data.as_ref());
							Some(Message::Batched {
								messages: Box::new([
									PortfolioMessage::DocumentPassMessage {
										document_id,
										message: ResourceMessage::Resolved { resource_id, source, hash }.into(),
									}
									.into(),
									ResourceStorageMessage::Store { data: Arc::from(data) }.into(),
								]),
							})
						}
						None => {
							log::warn!("Failed to fetch resource {resource_id} from {url}");
							None
						}
					}
				}

				responses.add(NetworkMessage::request(async move |client| {
					let mut loaded_catalog = None;
					let mut response: Option<Message> = None;
					for (source, hash) in sources {
						if let Some(hash) = hash {
							response = Some(ResourceMessage::Resolved { resource_id, source, hash }.into());
							break;
						}

						match &source {
							DataSource::Embedded => continue,
							DataSource::Url(url) => {
								response = resolve_to_message(document_id, resource_id, source.clone(), url.clone(), &client).await;
							}
							DataSource::Font { family, style } => {
								let font = match style {
									Some(style) => Font::new(family.clone(), style.clone()),
									None => Font::new_with_default_style(family.clone()),
								};

								if font_catalog.is_empty() && loaded_catalog.as_ref().is_none() {
									loaded_catalog = FontCatalog::load_from_api(&client).await;
								}

								let url = loaded_catalog.as_ref().and_then(|catalog| catalog.download_url(&font)).or_else(|| font_catalog.download_url(&font));

								if let Some(url) = url {
									let Ok(url) = Url::parse(&url) else {
										log::warn!("Invalid URL {url} for font resource {resource_id}");
										continue;
									};
									response = resolve_to_message(document_id, resource_id, source.clone(), url, &client).await;
								} else {
									log::warn!("No download URL found for font resource {resource_id}");
								}
							}
						}
						if response.is_some() {
							break;
						}
					}

					let mut response = response.unwrap_or_else(|| {
						log::error!("Resolve for {resource_id}: all sources exhausted");
						PortfolioMessage::DocumentPassMessage {
							document_id,
							message: ResourceMessage::ResolveFailed { resource_id }.into(),
						}
						.into()
					});

					if let Some(catalog) = loaded_catalog.take() {
						response = Message::Batched {
							messages: Box::new([response, FontsMessage::CatalogLoaded { catalog }.into()]),
						};
					}

					response
				}))
			}
			ResourceMessage::Resolved { resource_id, source, hash } => {
				self.pending_resolves.remove(&resource_id);
				if self.registry.info(&resource_id).is_none() {
					// Resource was removed from registry after the fetch started.
					return;
				}

				self.registry.resolve(&resource_id, hash);

				if let DataSource::Font { family, style } = source {
					let font = match style {
						Some(style) => Font::new(family, style),
						None => Font::new_with_default_style(family),
					};
					responses.add(FontsMessage::ResourceResolved { font, hash });
				}

				responses.add(NodeGraphMessage::RunDocumentGraph);
			}
			ResourceMessage::ResolveFailed { resource_id } => {
				self.pending_resolves.remove(&resource_id);
			}
		}
	}

	fn actions(&self) -> ActionList {
		actions!(ResourceMessageDiscriminant;)
	}
}

impl ResourceMessageHandler {
	pub fn is_empty(&self) -> bool {
		self.registry.is_empty() && self.embedded.is_empty()
	}

	pub async fn embed_resources(&mut self, resources_load_handle: Box<dyn LoadResource>) {
		let embedded = self
			.registry
			.resolved()
			.filter(|info| info.sources.contains(&DataSource::Embedded))
			.filter_map(|info| {
				if let Some(hash) = info.hash {
					let resource = resources_load_handle.load(*hash);
					Some(async move { resource.await.map(|resource| (*hash, resource)) })
				} else {
					None
				}
			})
			.collect::<Vec<_>>();

		self.embedded = EmbeddedResources::from_iter(futures::future::join_all(embedded).await.into_iter().flatten());
	}

	pub fn collect_garbage(&mut self, used: &[ResourceId]) {
		let used = HashSet::<ResourceId>::from_iter(used.iter().cloned());
		let unused = self.registry.ids().filter(|id| !used.contains(id)).collect::<Vec<_>>();
		unused.into_iter().for_each(|id| {
			self.registry.delete(&id);
		});
	}
}

// TODO: Eventually remove this document upgrade code
impl<'de> serde::Deserialize<'de> for ResourceMessageHandler {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		enum Key {
			Registry,
			Embedded,
			Hash(ResourceHash),
		}

		impl<'de> serde::Deserialize<'de> for Key {
			fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
				let raw = String::deserialize(deserializer)?;
				Ok(match raw.as_str() {
					"registry" => Key::Registry,
					"embedded" => Key::Embedded,
					_ => Key::Hash(raw.parse().map_err(serde::de::Error::custom)?),
				})
			}
		}

		struct EmbeddedResourcesVisitor {
			human_readable: bool,
		}

		impl<'de> serde::de::Visitor<'de> for EmbeddedResourcesVisitor {
			type Value = ResourceMessageHandler;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("an EmbeddedResources struct or a legacy EmbeddedResourceData map")
			}

			fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
				let mut output = ResourceMessageHandler::default();

				while let Some(key) = map.next_key::<Key>()? {
					match key {
						Key::Registry => output.registry = map.next_value()?,
						Key::Embedded => output.embedded = map.next_value()?,
						Key::Hash(hash) => {
							let bytes = if self.human_readable {
								let encoded: String = map.next_value()?;
								BASE64.decode(&encoded).map_err(serde::de::Error::custom)?
							} else {
								let raw: serde_bytes::ByteBuf = map.next_value()?;
								raw.into_vec()
							};
							let data_hash = output.embedded.store(Resource::new(bytes));
							if data_hash != hash {
								return Err(serde::de::Error::custom(format!("EmbeddedResource hash mismatch: expected {hash}, got {data_hash}")));
							}
						}
					}
				}

				Ok(output)
			}
		}

		let human_readable = deserializer.is_human_readable();
		deserializer.deserialize_map(EmbeddedResourcesVisitor { human_readable })
	}
}
