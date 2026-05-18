use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use graph_craft::application_io::{Resource, ResourceHash};
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct EmbeddedResources {
	resources: HashMap<ResourceHash, Resource>,
}

impl EmbeddedResources {
	pub fn is_empty(&self) -> bool {
		self.resources.is_empty()
	}

	pub fn store(&mut self, resource: Resource) -> ResourceHash {
		let hash = ResourceHash::from(resource.as_ref());
		self.resources.insert(hash, resource);
		hash
	}
}

impl FromIterator<(ResourceHash, Resource)> for EmbeddedResources {
	fn from_iter<T: IntoIterator<Item = (ResourceHash, Resource)>>(iter: T) -> Self {
		Self {
			resources: iter.into_iter().collect(),
		}
	}
}

impl IntoIterator for EmbeddedResources {
	type Item = (ResourceHash, Resource);
	type IntoIter = std::collections::hash_map::IntoIter<ResourceHash, Resource>;

	fn into_iter(self) -> Self::IntoIter {
		self.resources.into_iter()
	}
}

impl serde::Serialize for EmbeddedResources {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		use serde::ser::SerializeMap;

		let human_readable = serializer.is_human_readable();
		let mut map = serializer.serialize_map(Some(self.resources.len()))?;
		for (hash, resource) in &self.resources {
			let bytes: &[u8] = resource.as_ref();
			if human_readable {
				map.serialize_entry(hash, &BASE64.encode(bytes))?;
			} else {
				map.serialize_entry(hash, serde_bytes::Bytes::new(bytes))?;
			}
		}
		map.end()
	}
}

impl<'de> serde::Deserialize<'de> for EmbeddedResources {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		struct EmbeddedResourcesVisitor {
			human_readable: bool,
		}

		impl<'de> serde::de::Visitor<'de> for EmbeddedResourcesVisitor {
			type Value = EmbeddedResources;

			fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
				formatter.write_str("a map of ResourceHash to resource bytes")
			}

			fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
				let mut resources = HashMap::with_capacity(map.size_hint().unwrap_or(0));
				while let Some(hash) = map.next_key::<ResourceHash>()? {
					let resource = if self.human_readable {
						let encoded: String = map.next_value()?;
						let bytes = BASE64.decode(&encoded).map_err(serde::de::Error::custom)?;
						Resource::new(bytes)
					} else {
						let bytes: serde_bytes::ByteBuf = map.next_value()?;
						Resource::new(bytes.into_vec())
					};
					resources.insert(hash, resource);
				}
				Ok(EmbeddedResources { resources })
			}
		}

		let human_readable = deserializer.is_human_readable();
		deserializer.deserialize_map(EmbeddedResourcesVisitor { human_readable })
	}
}
