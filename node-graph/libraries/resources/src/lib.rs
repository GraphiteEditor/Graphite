use core_types::{CacheHash, graphene_hash};
use dyn_any::DynAny;
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::ops::Deref;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone, DynAny)]
pub struct Resource {
	inner: Arc<dyn AsRef<[u8]> + Send + Sync>,
	hash: ResourceHash,
}

impl Resource {
	pub fn new<T: AsRef<[u8]> + Send + Sync + 'static>(data: T) -> Self {
		let hash = ResourceHash::from(data.as_ref());
		Self { inner: Arc::new(data), hash }
	}

	pub fn new_unchecked<T: AsRef<[u8]> + Send + Sync + 'static>(data: T, hash: ResourceHash) -> Self {
		Self { inner: Arc::new(data), hash }
	}

	pub fn hash(&self) -> ResourceHash {
		self.hash
	}
}

impl From<&Resource> for Arc<dyn AsRef<[u8]> + Send + Sync> {
	fn from(val: &Resource) -> Self {
		val.inner.clone()
	}
}

impl Deref for Resource {
	type Target = [u8];

	fn deref(&self) -> &[u8] {
		(*self.inner).as_ref()
	}
}

impl AsRef<[u8]> for Resource {
	fn as_ref(&self) -> &[u8] {
		(*self.inner).as_ref()
	}
}

impl std::fmt::Debug for Resource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Resource").field("len", &self.len()).finish()
	}
}

impl PartialEq for Resource {
	fn eq(&self, other: &Self) -> bool {
		self.hash == other.hash
	}
}

impl CacheHash for Resource {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.hash.cache_hash(state);
	}
}

/// Blake3 content hash of a resource, represented as 32 bytes
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord, DynAny)]
pub struct ResourceHash([u8; 32]);

impl From<&[u8]> for ResourceHash {
	fn from(data: &[u8]) -> Self {
		Self(blake3::hash(data).into())
	}
}

impl From<[u8; 32]> for ResourceHash {
	fn from(bytes: [u8; 32]) -> Self {
		Self(bytes)
	}
}

impl From<&ResourceHash> for [u8; 32] {
	fn from(hash: &ResourceHash) -> Self {
		hash.0
	}
}

impl From<&ResourceHash> for String {
	fn from(hash: &ResourceHash) -> Self {
		const HEX: &[u8; 16] = b"0123456789abcdef";
		let mut out = String::with_capacity(hash.0.len() * 2);
		for byte in &hash.0 {
			out.push(HEX[(byte >> 4) as usize] as char);
			out.push(HEX[(byte & 0x0f) as usize] as char);
		}
		out
	}
}

impl std::fmt::Display for ResourceHash {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&String::from(self))
	}
}

impl std::str::FromStr for ResourceHash {
	type Err = ResourceHashParseError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		fn decode_hex_nibble(byte: u8, position: usize) -> Result<u8, ResourceHashParseError> {
			match byte {
				b'0'..=b'9' => Ok(byte - b'0'),
				b'a'..=b'f' => Ok(byte - b'a' + 10),
				b'A'..=b'F' => Ok(byte - b'A' + 10),
				_ => Err(ResourceHashParseError::InvalidCharacter { byte, position }),
			}
		}

		let bytes = s.as_bytes();
		if bytes.len() != 64 {
			return Err(ResourceHashParseError::InvalidLength { found: bytes.len() });
		}

		let mut out = [0u8; 32];
		for (index, chunk) in bytes.chunks_exact(2).enumerate() {
			let high = decode_hex_nibble(chunk[0], index * 2)?;
			let low = decode_hex_nibble(chunk[1], index * 2 + 1)?;
			out[index] = (high << 4) | low;
		}

		Ok(Self(out))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceHashParseError {
	InvalidLength { found: usize },
	InvalidCharacter { byte: u8, position: usize },
}

impl std::fmt::Display for ResourceHashParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::InvalidLength { found } => write!(f, "resource hash must be 64 hex characters, got {found}"),
			Self::InvalidCharacter { byte, position } => write!(f, "resource hash contains non-hex byte {byte:#04x} at position {position}"),
		}
	}
}

impl std::error::Error for ResourceHashParseError {}

impl TryFrom<&str> for ResourceHash {
	type Error = ResourceHashParseError;

	fn try_from(value: &str) -> Result<Self, Self::Error> {
		value.parse()
	}
}

#[cfg(feature = "serde")]
impl serde::Serialize for ResourceHash {
	fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		if serializer.is_human_readable() {
			serializer.serialize_str(&String::from(self))
		} else {
			serializer.serialize_bytes(&self.0)
		}
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ResourceHash {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		struct ResourceHashVisitor;

		impl<'de> serde::de::Visitor<'de> for ResourceHashVisitor {
			type Value = ResourceHash;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("a 64-character hex string or 32 raw bytes")
			}

			fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
				ResourceHash::try_from(value).map_err(E::custom)
			}

			fn visit_bytes<E: serde::de::Error>(self, value: &[u8]) -> Result<Self::Value, E> {
				let bytes: [u8; 32] = value.try_into().map_err(|_| E::invalid_length(value.len(), &"32 bytes"))?;
				Ok(ResourceHash(bytes))
			}

			fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
				let mut bytes = [0u8; 32];
				for (i, slot) in bytes.iter_mut().enumerate() {
					*slot = seq.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(i, &"32 bytes"))?;
				}
				Ok(ResourceHash(bytes))
			}
		}

		if deserializer.is_human_readable() {
			deserializer.deserialize_str(ResourceHashVisitor)
		} else {
			deserializer.deserialize_bytes(ResourceHashVisitor)
		}
	}
}

impl CacheHash for ResourceHash {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(self, state);
	}
}

pub trait LoadResource: Send + Sync {
	fn load(&self, hash: ResourceHash) -> ResourceFuture;
}

pub type ResourceFuture = Pin<Box<dyn Future<Output = Option<Resource>> + Send + 'static>>;

pub trait ResourceStorage: LoadResource {
	fn store(&mut self, data: &[u8]) -> ResourceHash;
	fn contains(&mut self, hash: &ResourceHash) -> bool;
	fn garbage_collect(&mut self, used: &[ResourceHash]);
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, graphene_hash::CacheHash, PartialOrd, Ord, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceId(u64);

impl ResourceId {
	pub fn new() -> Self {
		Self(core_types::uuid::generate_uuid())
	}
}

impl std::fmt::Display for ResourceId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

pub type DataSources = Box<[DataSource]>;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataSource {
	Embedded,
	Url(url::Url),
	Font { family: String, style: Option<String> },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResourceRegistry {
	hashes: HashMap<ResourceId, ResourceHash>,
	sources: HashMap<ResourceId, Vec<DataSource>>,
}

impl ResourceRegistry {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn is_empty(&self) -> bool {
		self.hashes.is_empty() && self.sources.is_empty()
	}

	pub fn contains(&self, id: &ResourceId) -> bool {
		self.hashes.contains_key(id) || self.sources.contains_key(id)
	}

	pub fn ids(&self) -> impl Iterator<Item = ResourceId> + '_ {
		self.hashes.keys().chain(self.sources.keys().filter(|id| !self.hashes.contains_key(id))).copied()
	}

	pub fn info(&self, id: &ResourceId) -> Option<ResourceInfo<'_>> {
		self.contains(id).then(|| ResourceInfo {
			id: *id,
			hash: self.hashes.get(id),
			sources: self.sources.get(id).map(|sources| sources.as_slice()).unwrap_or(&[]),
		})
	}

	pub fn push_source_back(&mut self, id: &ResourceId, source: DataSource) {
		self.sources.entry(*id).or_default().push(source);
	}

	pub fn push_source_front(&mut self, id: &ResourceId, source: DataSource) {
		self.sources.entry(*id).or_default().insert(0, source);
	}

	pub fn delete(&mut self, id: &ResourceId) -> bool {
		let hash = self.hashes.remove(id);
		let sources = self.sources.remove(id);
		!(hash.is_none() && sources.is_none())
	}

	pub fn resolve(&mut self, id: &ResourceId, hash: ResourceHash) -> Option<ResourceHash> {
		self.hashes.insert(*id, hash)
	}

	pub fn hash(&self, id: &ResourceId) -> Option<ResourceHash> {
		self.hashes.get(id).copied()
	}

	pub fn unresolved(&self) -> impl Iterator<Item = ResourceInfo<'_>> + '_ {
		self.sources.keys().filter(|id| !self.hashes.contains_key(id)).filter_map(|id| self.info(id))
	}

	pub fn resolved(&self) -> impl Iterator<Item = ResourceInfo<'_>> + '_ {
		self.hashes.keys().filter_map(|id| self.info(id))
	}
}

#[derive(Clone, Debug)]
pub struct ResourceInfo<'a> {
	pub id: ResourceId,
	pub hash: Option<&'a ResourceHash>,
	pub sources: &'a [DataSource],
}
