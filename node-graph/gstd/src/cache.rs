use parking_lot::RawRwLock;
use std::any::Any;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::iter::{self, Sum};
use std::marker::PhantomData;
use storage_map::{StorageMap, StorageMapGuard};

/// Caches the output of a given Node and acts as a proxy
/// Automatically resets if it receives different input
pub struct SmartCacheNode<'n, 'c, NODE: Node + 'c> {
	node: &'n NODE,
	map: StorageMap<RawRwLock, HashMap<u64, CacheNode<'n, 'c, NODE>>>,
}
impl<'n: 'c, 'c, NODE: Node + 'c> Node for SmartCacheNode<'n, 'c, NODE>
where
	for<'a> NODE::Input<'a>: Hash,
{
	type Input<'a>
		= NODE::Input<'a>
	where
		Self: 'a,
		'c: 'a;
	type Output<'a>
		= StorageMapGuard<'a, RawRwLock, CacheNode<'n, 'c, NODE>>
	where
		Self: 'a,
		'c: 'a;
	fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
		let mut hasher = DefaultHasher::new();
		input.borrow().hash(&mut hasher);
		let hash = hasher.finish();

		self.map.get_or_create_with(&hash, || {
			trace!("Creating new cache node");
			CacheNode::new(self.node)
		})
	}
}

impl<'n, 'c, NODE: Node> SmartCacheNode<'n, 'c, NODE> {
	pub fn clear(&'n mut self) {
		self.map = StorageMap::default();
	}
	pub fn new(node: &'n NODE) -> SmartCacheNode<'n, 'c, NODE> {
		SmartCacheNode { node, map: StorageMap::default() }
	}
}
