use crate::messages::portfolio::document::resource::utility_types::EmbeddedResources;
use crate::messages::prelude::*;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use graph_craft::application_io::resource::{DataSource, LoadResource, Resource, ResourceHash, ResourceId, ResourceRegistry};

#[derive(ExtractField)]
pub struct ResourceMessageContext {}

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, ExtractField)]
pub struct ResourceMessageHandler {
	pub registry: ResourceRegistry,
	pub embedded: EmbeddedResources,
}

#[message_handler_data]
impl MessageHandler<ResourceMessage, ResourceMessageContext> for ResourceMessageHandler {
	fn process_message(&mut self, message: ResourceMessage, responses: &mut VecDeque<Message>, _context: ResourceMessageContext) {
		match message {
			ResourceMessage::StoreEmbedded { resource_id, data } => {
				let hash = ResourceHash::from(data.as_ref());
				self.registry.push_source_back(&resource_id, DataSource::Embedded);
				self.registry.resolve(&resource_id, hash);
				responses.add(ResourceStorageMessage::Store { data });
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

	pub fn garbage_collect(&mut self, used: &[ResourceId]) {
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
