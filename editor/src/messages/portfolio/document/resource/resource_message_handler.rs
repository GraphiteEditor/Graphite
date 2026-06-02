use crate::messages::portfolio::document::resource::utility_types::EmbeddedResources;
use crate::messages::prelude::*;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use graph_craft::application_io::resource::{DataSource, LoadResource, Resource, ResourceHash, ResourceId, ResourceRegistry};
use graphene_std::text::Font;
use std::sync::Arc;

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
	pending_resolves: HashMap<ResourceId, Option<ResolveProgress>>,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolveProgress {
	index: usize,
	source: DataSource,
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
				responses.add(ResourceMessage::Resolve);
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
				responses.add(ResourceMessage::Resolve);
			}
			ResourceMessage::Resolve => {
				let unresolved_ids: Vec<ResourceId> = self.registry.unresolved().map(|info| info.id).collect();
				for id in unresolved_ids {
					if self.pending_resolves.contains_key(&id) {
						continue;
					}
					self.pending_resolves.insert(id, None);
					responses.add(ResourceMessage::ResolveStep { resource_id: id });
				}
			}
			ResourceMessage::ResolveStep { resource_id } => {
				let Some(progress) = self.pending_resolves.get_mut(&resource_id) else { return };

				let Some(info) = self.registry.info(&resource_id) else {
					log::error!("ResolveStep for {resource_id}: no registry entry");
					self.pending_resolves.remove(&resource_id);
					return;
				};

				let index = if let Some(progress) = progress { progress.index + 1 } else { 0 };
				let Some(source) = info.sources.get(index).cloned() else {
					log::error!("ResolveStep for {resource_id}: no more sources to try");
					self.pending_resolves.remove(&resource_id);
					return;
				};
				*progress = Some(ResolveProgress { index, source: source.clone() });

				match source {
					DataSource::Embedded => {
						// Embedded resources are loaded on document load.
						// If we get to this point, it means the resource was not embedded and we should try the next source.
						responses.add(ResourceMessage::ResolveStep { resource_id });
					}
					DataSource::Url(url) => {
						responses.add(fetch_resource(document_id, resource_id, url.to_string()));
					}
					DataSource::Font { family, style } => {
						let font = match style {
							Some(style) => Font::new(family, style),
							None => Font::new_with_default_style(family),
						};
						if let Some(hash) = fonts.cached_hash(&font) {
							self.registry.resolve(&resource_id, hash);
							self.pending_resolves.remove(&resource_id);
							responses.add(NodeGraphMessage::RunDocumentGraph);
							return;
						}
						if let Some(url) = fonts.cached_url(&font) {
							responses.add(fetch_resource(document_id, resource_id, url));
							return;
						}
						responses.add(FontsMessage::LoadCatalog);
						self.pending_resolves.remove(&resource_id);
					}
				}
			}
			ResourceMessage::Resolved { resource_id, data } => {
				let hash = ResourceHash::from(data.as_ref());
				let Some(progress) = self.pending_resolves.remove(&resource_id).and_then(|p| p) else {
					log::error!("Resolved message for {resource_id} with no pending resolve");
					return;
				};
				let Some(info) = self.registry.info(&resource_id) else {
					// ResourceId was removed from registry after resolve started.
					// This can happen if the document was modified while resolves were in-flight.
					// Likely safe to ignore for now.
					// TODO: Consider adding cleaner cancelation for in-flight resolves.
					return;
				};
				let Some(source) = info.sources.get(progress.index).cloned() else {
					log::error!("Resolved message for {resource_id} with no current source");
					return;
				};
				if progress.source != source {
					log::error!("Resolved message for {resource_id} with mismatched source");
					return;
				}

				self.registry.resolve(&resource_id, hash);
				responses.add(ResourceStorageMessage::Store { data });

				if let DataSource::Font { family, style } = source {
					let font = match style {
						Some(style) => Font::new(family, style),
						None => Font::new_with_default_style(family),
					};
					responses.add(FontsMessage::ResourceResolved { font, hash });
				}

				responses.add(ResourceMessage::Resolve);
				responses.add(NodeGraphMessage::RunDocumentGraph);
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

fn fetch_resource(document_id: DocumentId, resource_id: ResourceId, url: String) -> Message {
	NetworkMessage::request(move |client| async move {
		let Some(bytes) = client.fetch(&url).await else { return Message::NoOp };
		PortfolioMessage::DocumentPassMessage {
			document_id,
			message: DocumentMessage::Resource(ResourceMessage::Resolved {
				resource_id,
				data: Arc::from(bytes),
			}),
		}
		.into()
	})
	.into()
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
