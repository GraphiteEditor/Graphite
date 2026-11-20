//! Format version 1, frozen by `cargo run migration snapshot`. Never edit it: old documents decode through these definitions.

use graphene_resource::{ResourceHash, ResourceId};
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeValue {
	pub value: Value,
	pub timestamp: TimeStamp,
	#[serde(default)]
	pub deleted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Attributes {
	pub entries: BTreeMap<String, AttributeValue>,
	#[serde(default)]
	pub floor: TimeStamp,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delta {
	pub id: Rev,
	pub parent: Option<Rev>,
	pub author: PeerId,
	pub timestamp: TimeStamp,
	pub kind: RegistryDelta,
	pub reverse: Vec<Prior>,
	#[serde(default)]
	pub attributes: Attributes,
	#[serde(default)]
	pub retired_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RegistryDelta {
	AddNode { id: NodeId, node: Node },
	RemoveNode { id: NodeId, snapshot: Node },
	ChangeNodeInput { id: NodeId, index: u32, new_input: NodeInput },
	SetNodeInputs { id: NodeId, inputs: Vec<InputSlot> },
	SetNodeImplementation { id: NodeId, implementation: Implementation },
	ChangeNodeAttribute { id: NodeId, delta: AttributeDelta },
	ChangeNodeInputAttribute { id: NodeId, index: u32, delta: AttributeDelta },
	SetNetworkExport { id: NetworkId, index: u32, export: Option<NodeInput> },
	ChangeNetworkAttribute { id: NetworkId, delta: AttributeDelta },
	AddNetwork { id: NetworkId, network: Network },
	RemoveNetwork { id: NetworkId, snapshot: Network },
	AddResource { id: ResourceId, entry: ResourceEntry },
	SetResourceHash { id: ResourceId, hash: Option<ResourceHash> },
	RemoveResource { id: ResourceId, snapshot: ResourceEntry },
	AddSource { id: ResourceId, key: SourceKey, source: Value },
	RemoveSource { id: ResourceId, key: SourceKey },
	RegisterPeer { peer: PeerId, user: UserId },
	ChangeDocumentAttribute { delta: AttributeDelta },
	Merge { extra_parents: Vec<Rev> },
	Meta,
	Other(Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeDelta {
	pub key: String,
	pub value: Option<Value>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NetworkId(pub u64);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rev(pub std::num::NonZeroU128);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PeerId(pub u64);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u64);

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct TimeStamp {
	pub counter: u64,
	pub peer: PeerId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
	#[serde(default)]
	pub presence: TimeStamp,
	pub network: NetworkId,
	#[serde(default)]
	pub network_timestamp: TimeStamp,
	pub implementation: Implementation,
	#[serde(default)]
	pub implementation_timestamp: TimeStamp,
	pub inputs: Vec<InputSlot>,
	#[serde(default)]
	pub inputs_timestamp: TimeStamp,
	pub attributes: Attributes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputSlot {
	pub input: NodeInput,
	pub timestamp: TimeStamp,
	pub attributes: Attributes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeInput {
	Node { id: NodeId, index: u32 },
	Value { value: Value, exposed: bool },
	Scope(Cow<'static, str>),
	Import { index: u32 },
	Reflection,
	Other,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Implementation {
	ProtoNode(ResourceId),
	Network(NetworkId),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Network {
	#[serde(default)]
	pub presence: TimeStamp,
	pub exports: Vec<ExportSlot>,
	#[serde(default)]
	pub exports_timestamp: TimeStamp,
	pub attributes: Attributes,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExportSlot {
	pub target: Option<NodeInput>,
	pub timestamp: TimeStamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProtoNode {
	pub identifier: String,
	pub attributes: Attributes,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Prior {
	Node { id: NodeId, node: Node },
	RemovedNode { id: NodeId, tombstone: Tombstone<Node> },
	AbsentNode { id: NodeId },
	Network { id: NetworkId, network: Network },
	RemovedNetwork { id: NetworkId, tombstone: Tombstone<Network> },
	AbsentNetwork { id: NetworkId },
	Resource { id: ResourceId, resource: ResourceEntry },
	RemovedResource { id: ResourceId, tombstone: Tombstone<ResourceEntry> },
	AbsentResource { id: ResourceId },
	NodeField { id: NodeId, presence: TimeStamp, field: NodeField },
	NetworkField { id: NetworkId, presence: TimeStamp, field: NetworkField },
	DocumentAttribute { key: String, previous: Option<AttributeValue> },
	PeerUser { peer: PeerId, previous: Option<PeerRegistration> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NodeField {
	Presence,
	Attribute { key: String, previous: Option<AttributeValue> },
	Input { index: u32, previous: InputSlot },
	InputAttribute { index: u32, key: String, previous: Option<AttributeValue> },
	Inputs { previous: Vec<InputSlot>, inputs_timestamp: TimeStamp },
	Implementation { previous: Implementation, implementation_timestamp: TimeStamp },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetworkField {
	Presence,
	Attribute { key: String, previous: Option<AttributeValue> },
	Exports { previous: Vec<ExportSlot> },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Registry {
	pub node_instances: HashMap<NodeId, Node>,
	pub networks: HashMap<NetworkId, Network>,
	pub resources: ResourceStore,
	pub peer_users: HashMap<PeerId, PeerRegistration>,
	pub attributes: Attributes,
	#[serde(default)]
	pub removed_nodes: HashMap<NodeId, Tombstone<Node>>,
	#[serde(default)]
	pub removed_networks: HashMap<NetworkId, Tombstone<Network>>,
	#[serde(default)]
	pub removed_resources: HashMap<ResourceId, Tombstone<ResourceEntry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRegistration {
	pub user: UserId,
	pub timestamp: TimeStamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tombstone<T> {
	pub content: T,
	pub timestamp: TimeStamp,
	#[serde(default)]
	pub placeholder: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourceKey {
	pub priority: Priority,
	pub peer: PeerId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceValue {
	pub source: Value,
	pub timestamp: TimeStamp,
	#[serde(default)]
	pub deleted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ResourceEntry {
	pub presence: TimeStamp,
	pub sources: Vec<(SourceKey, SourceValue)>,
	pub sources_timestamp: TimeStamp,
	pub hash: Option<ResourceHash>,
	pub hash_timestamp: TimeStamp,
}

impl<'de> Deserialize<'de> for ResourceEntry {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		#[derive(Deserialize)]
		struct Raw {
			#[serde(default)]
			presence: TimeStamp,
			sources: Vec<(SourceKey, SourceValue)>,
			#[serde(default)]
			sources_timestamp: TimeStamp,
			hash: Option<ResourceHash>,
			hash_timestamp: TimeStamp,
		}
		let Raw {
			presence,
			mut sources,
			sources_timestamp,
			hash,
			hash_timestamp,
		} = Raw::deserialize(deserializer)?;
		sources.sort_by_key(|(key, _)| *key);
		sources.dedup_by(|(later_key, later_value), (kept_key, kept_value)| {
			if later_key != kept_key {
				return false;
			}
			if later_value.timestamp > kept_value.timestamp {
				*kept_value = later_value.clone();
			}
			true
		});
		Ok(Self {
			presence,
			sources,
			sources_timestamp,
			hash,
			hash_timestamp,
		})
	}
}

pub type ResourceStore = HashMap<ResourceId, ResourceEntry>;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(try_from = "f64")]
pub struct Priority(pub f64);

impl Priority {
	pub fn new(value: f64) -> Result<Self, NonFinitePriority> {
		if value.is_finite() { Ok(Self(value)) } else { Err(NonFinitePriority(value)) }
	}
	pub fn value(self) -> f64 {
		self.0
	}
}

impl TryFrom<f64> for Priority {
	type Error = NonFinitePriority;
	fn try_from(value: f64) -> Result<Self, Self::Error> {
		Self::new(value)
	}
}

impl PartialEq for Priority {
	fn eq(&self, other: &Self) -> bool {
		self.cmp(other) == std::cmp::Ordering::Equal
	}
}

impl Eq for Priority {}

impl Ord for Priority {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.0.total_cmp(&other.0)
	}
}

impl PartialOrd for Priority {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl std::hash::Hash for Priority {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.0.to_bits().hash(state);
	}
}

#[derive(Debug, thiserror :: Error)]
#[error("priority must be finite, got {0}")]
pub struct NonFinitePriority(pub f64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotOp {
	pub op: RegistryDelta,
	pub timestamp: TimeStamp,
	pub sequence: HotSequence,
	#[serde(default)]
	pub attributes: Attributes,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HotSequence(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HotOpId {
	pub peer: PeerId,
	pub sequence: HotSequence,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredMarks {
	pub retired_up_to: HashMap<PeerId, HotSequence>,
	pub retired_runs: HashMap<PeerId, Vec<(HotSequence, HotSequence)>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionState {
	#[serde(default)]
	pub peer_id: PeerId,
	#[serde(default)]
	pub user_id: UserId,
	#[serde(default)]
	pub head_rev: Option<Rev>,
	#[serde(default)]
	pub last_broadcast_rev: Option<Rev>,
	#[serde(default)]
	pub redo_stack: Vec<Rev>,
	#[serde(default)]
	pub next_node_counter: u64,
	#[serde(default)]
	pub last_hot_sequence: HotSequence,
	#[serde(default)]
	pub view_settings: BTreeMap<String, Value>,
	#[serde(default)]
	pub network_view_settings: BTreeMap<NetworkId, BTreeMap<String, Value>>,
	#[serde(default)]
	pub retired_marks: RetiredMarks,
	#[serde(default)]
	pub clock_counter: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub enum Value {
	None,
	Bool(bool),
	Int(i128),
	Float(f64),
	Str(String),
	Array(Vec<Value>),
	Object(Vec<(String, Value)>),
}

impl Serialize for Value {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		if !serializer.is_human_readable() {
			return Value::serialize(self, serializer);
		}
		match self {
			Value::None => serializer.serialize_unit(),
			Value::Bool(boolean) => serializer.serialize_bool(*boolean),
			Value::Int(int) => serializer.serialize_i128(*int),
			Value::Float(float) => serializer.serialize_f64(*float),
			Value::Str(string) => serializer.serialize_str(string),
			Value::Array(array) => serializer.collect_seq(array),
			Value::Object(object) => serializer.collect_map(object.iter().map(|(key, value)| (key, value))),
		}
	}
}

impl<'de> Deserialize<'de> for Value {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		if !deserializer.is_human_readable() {
			return Value::deserialize(deserializer);
		}
		struct Untagged;
		impl<'de> Visitor<'de> for Untagged {
			type Value = Value;
			fn visit_unit<E>(self) -> Result<Value, E> {
				Ok(Value::None)
			}
			fn visit_none<E>(self) -> Result<Value, E> {
				Ok(Value::None)
			}
			fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
				Deserialize::deserialize(deserializer)
			}
			fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
				Ok(Value::Bool(value))
			}
			fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
				Ok(Value::Int(value.into()))
			}
			fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
				Ok(Value::Int(value.into()))
			}
			fn visit_i128<E>(self, value: i128) -> Result<Value, E> {
				Ok(Value::Int(value))
			}
			fn visit_u128<E: de::Error>(self, value: u128) -> Result<Value, E> {
				i128::try_from(value).map(Value::Int).map_err(E::custom)
			}
			fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
				Ok(Value::Float(value))
			}
			fn visit_str<E>(self, value: &str) -> Result<Value, E> {
				Ok(Value::Str(value.to_owned()))
			}
			fn visit_string<E>(self, value: String) -> Result<Value, E> {
				Ok(Value::Str(value))
			}
			fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
				let mut array = Vec::with_capacity(seq.size_hint().unwrap_or(0));
				while let Some(value) = seq.next_element()? {
					array.push(value);
				}
				Ok(Value::Array(array))
			}
			fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
				let mut object: Vec<(String, Value)> = Vec::with_capacity(map.size_hint().unwrap_or(0));
				while let Some(entry) = map.next_entry()? {
					object.push(entry);
				}
				object.sort_by(|a, b| a.0.cmp(&b.0));
				object.dedup_by(|b, a| {
					let dedup = b.0 == a.0;
					if dedup {
						std::mem::swap(&mut a.1, &mut b.1);
					}
					dedup
				});
				Ok(Value::Object(object))
			}
			fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
				formatter.write_str("a valid Value")
			}
		}
		deserializer.deserialize_any(Untagged)
	}
}
