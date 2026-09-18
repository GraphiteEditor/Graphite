//! Batch evaluation over a flat lane range without building a context per
//! lane: a [`LaneMap`] decomposes a global lane into index levels, and a
//! [`Lane`] answers the context traits from the map and the batch's fixed
//! part on demand.

use crate::arena::Arena;
use crate::context::{
	ContextFeatures, ContextImpl, Ctx, EvalScope, ExtractAnimationTime, ExtractArena, ExtractFootprint, ExtractIndices, ExtractPointerPosition, ExtractPosition, ExtractRealTime, ExtractVarArgs,
	IndexLevels, IndexLink, InjectIndex, PositionLink, VarArgLink, VarArgsResult,
};
use crate::transform::Footprint;
use glam::DVec2;
use std::hash::{Hash, Hasher};

/// The levels a map can split a lane into; deeper nesting stays on the outer chain.
pub const MAX_LEVELS: usize = 8;

/// How a global lane decomposes into index levels, innermost first: level
/// `i` runs over `extents[i]` values with the product of the levels below
/// it as its stride. A reversed level counts down, as Repeat's `reverse`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LaneMap {
	extents: [u64; MAX_LEVELS],
	depth: u8,
	reversed: u8,
}

impl LaneMap {
	/// One level of `extent` lanes: the flat view of a level.
	pub fn flat(extent: u64) -> Self {
		let mut extents = [1; MAX_LEVELS];
		extents[0] = extent;
		Self { extents, depth: 1, reversed: 0 }
	}

	/// A flat view whose extent is unknown: a range over it is what the caller
	/// asked for, and refinement leaves the copy count open too.
	pub fn open() -> Self {
		Self::flat(u64::MAX)
	}

	pub fn depth(&self) -> usize {
		self.depth as usize
	}

	/// The number of lanes the map spans.
	pub fn total(&self) -> u64 {
		self.extents[..self.depth()].iter().product()
	}

	/// Splits level 0 into `inner` lanes per copy: the map reads `[inner, copies, ..]`
	/// afterwards. `None` where level 0 is not a whole number of copies or the
	/// map is full.
	pub fn refined(&self, inner: u64, reverse: bool) -> Option<Self> {
		// A reversed flat level has no well-defined split, so it refuses.
		let open = self.extents[0] == u64::MAX;
		if inner == 0 || self.depth() == MAX_LEVELS || (!open && self.extents[0] % inner != 0) || self.reversed & 1 == 1 {
			return None;
		}
		let mut map = *self;
		map.extents.copy_within(1..self.depth(), 2);
		map.extents[0] = inner;
		map.extents[1] = match open {
			true => u64::MAX,
			false => self.extents[0] / inner,
		};
		map.depth += 1;
		// The copies take the flag; the new inner level counts up.
		map.reversed = (self.reversed << 1) | (u8::from(reverse) << 1);
		Some(map)
	}

	/// Level `level`'s index at `lane`.
	pub fn level(&self, lane: u64, level: usize) -> u64 {
		let mut rest = lane;
		for extent in &self.extents[..level] {
			rest /= extent;
		}
		let index = rest % self.extents[level];
		match self.reversed >> level & 1 {
			1 => self.extents[level] - 1 - index,
			_ => index,
		}
	}
}

/// What every lane of a batch shares: the evaluation scope, the features a
/// context carries by reference, and the index chain above the map.
#[derive(Clone, Copy)]
pub struct Fixed<'a> {
	scope: &'a EvalScope<'a>,
	footprint: Option<&'a Footprint>,
	varargs: Option<&'a VarArgLink<'a>>,
	position: Option<&'a PositionLink<'a>>,
	outer: Option<&'a IndexLink<'a>>,
}

/// A batch: the fixed part, the map, the lanes asked for, and which levels
/// survive nullification. Structure nodes refine the map; boundaries narrow
/// the retained levels; kernels loop [`Lane`]s over the range.
#[derive(Clone)]
pub struct Dispatch<'a> {
	fixed: Fixed<'a>,
	map: LaneMap,
	retained: IndexLevels,
	range: std::ops::Range<u64>,
}

impl<'a> Dispatch<'a> {
	/// A dispatch over `range` of the level `ctx` addresses at its lane: the
	/// context's own lane becomes the map's level 0.
	pub fn new(ctx: &ContextImpl<'a>, map: LaneMap, range: std::ops::Range<u64>) -> Self {
		let head = ctx.index_head();
		Self {
			fixed: Fixed {
				scope: ctx.scope(),
				footprint: ctx.footprint_ref(),
				varargs: ctx.varargs_ref(),
				position: ctx.position_ref(),
				outer: head.outer,
			},
			map,
			retained: IndexLevels::all(),
			range,
		}
	}

	pub fn map(&self) -> &LaneMap {
		&self.map
	}

	pub fn range(&self) -> std::ops::Range<u64> {
		self.range.clone()
	}

	pub fn retained(&self) -> IndexLevels {
		self.retained
	}

	/// The same lanes with level 0 split into `inner` lanes per copy.
	pub fn refined(&self, inner: u64, reverse: bool) -> Option<Self> {
		Some(Self {
			map: self.map.refined(inner, reverse)?,
			retained: self.retained.split_innermost(),
			..self.clone()
		})
	}

	/// The same lanes with only `keep`'s levels readable.
	pub fn nullified(&self, keep: IndexLevels) -> Self {
		let mut retained = self.retained;
		retained &= keep;
		Self { retained, ..self.clone() }
	}

	pub fn lane(&self, lane: u64) -> Lane<'_> {
		Lane { dispatch: self, lane }
	}
}

/// The bridge between a kernel's context type and the batch: a context
/// yields the dispatch over a range of the level it addresses, and rebuilds
/// itself at any lane of a dispatch.
pub trait AsDispatch<'e>: Sized {
	fn dispatch(&self, map: LaneMap, range: std::ops::Range<u64>) -> Dispatch<'e>;
	/// The context at `lane`, or `None` when the arena cannot hold its chain.
	fn at_lane(dispatch: &Dispatch<'e>, lane: u64) -> Option<Self>;
}

impl<'e> AsDispatch<'e> for ContextImpl<'e> {
	fn dispatch(&self, map: LaneMap, range: std::ops::Range<u64>) -> Dispatch<'e> {
		Dispatch::new(self, map, range)
	}

	fn at_lane(dispatch: &Dispatch<'e>, lane: u64) -> Option<Self> {
		dispatch.serve_at(lane)
	}
}

impl<'e> Dispatch<'e> {
	pub fn scope(&self) -> &'e EvalScope<'e> {
		self.fixed.scope
	}

	/// The same lanes under another scope, as a boundary narrows the scope.
	pub fn with_scope<'s>(&self, scope: &'s EvalScope<'s>) -> Dispatch<'s>
	where
		'e: 's,
	{
		Dispatch {
			fixed: Fixed {
				scope,
				footprint: self.fixed.footprint,
				varargs: self.fixed.varargs,
				position: self.fixed.position,
				outer: self.fixed.outer,
			},
			map: self.map,
			retained: self.retained,
			range: self.range.clone(),
		}
	}

	/// The same lanes carrying only the features in `keep`.
	pub fn keeping(&self, keep: ContextFeatures) -> Self {
		Dispatch {
			fixed: Fixed {
				footprint: self.fixed.footprint.filter(|_| keep.contains(ContextFeatures::FOOTPRINT)),
				varargs: self.fixed.varargs.filter(|_| keep.contains(ContextFeatures::VARARGS)),
				position: self.fixed.position.filter(|_| keep.contains(ContextFeatures::POSITION)),
				..self.fixed
			},
			..self.clone()
		}
	}

	/// The key every lane of the dispatch shares: the context with the map's
	/// levels zeroed, which is what a memo keys a published level on.
	pub fn key(&self) -> u64 {
		let mut outer_only = IndexLevels::empty();
		for level in self.map.depth()..32 {
			outer_only = outer_only.with_level(level as u8);
		}
		crate::registry::cache_key(&self.nullified(outer_only).lane(0))
	}

	/// The serve-side context at `lane`, its canonical chain built in the
	/// scope's arena so it lives as long as the dispatch does.
	pub fn serve_at(&self, lane: u64) -> Option<ContextImpl<'e>> {
		let view = self.lane(lane);
		// A chain is a handful of levels, so it lands in a stack buffer; deeper
		// chains spill to a heap buffer rather than to a limit.
		let mut buffer = [0u64; 16];
		let mut spill = Vec::new();
		let mut count = 0;
		for level in view.canonical() {
			match count < buffer.len() {
				true => buffer[count] = level,
				false => spill.push(level),
			}
			count += 1;
		}
		let levels = |index: usize| match index < buffer.len() {
			true => buffer[index],
			false => spill[index - buffer.len()],
		};
		let arena = self.fixed.scope.arena();
		let mut outer: Option<&'e IndexLink<'e>> = None;
		for index in (1..count).rev() {
			outer = Some(arena.alloc(IndexLink { index: levels(index), outer })?.0);
		}
		let mut base = ContextImpl::root(self.fixed.scope);
		if let Some(footprint) = self.fixed.footprint {
			base = base.with_footprint(footprint);
		}
		if let Some(varargs) = self.fixed.varargs {
			base = base.with_varargs(varargs);
		}
		if let Some(position) = self.fixed.position {
			base = base.with_position(position);
		}
		Some(match outer {
			Some(outer) => base.promoted(outer, levels(0)),
			None => base.replaced(levels(0)),
		})
	}
}

impl LaneMap {
	/// Whether level 0's extent is still open.
	pub fn is_open(&self) -> bool {
		self.extents[0] == u64::MAX
	}

	/// The map with level 0 sized to `extent` where it was open.
	pub fn sized(&self, extent: u64) -> Self {
		let mut map = *self;
		if map.is_open() {
			map.extents[0] = extent;
		}
		map
	}

	/// The map with level 0 replaced by `extent`: a node whose own lane count
	/// differs from an input's hands that input the map it serves under.
	pub fn resized(&self, extent: u64) -> Self {
		let mut map = *self;
		map.extents[0] = extent;
		map
	}

	/// Level `level`'s extent, `u64::MAX` where open.
	pub fn extent(&self, level: usize) -> u64 {
		self.extents[level]
	}

	/// Whether level `level` counts down.
	pub fn is_reversed(&self, level: usize) -> bool {
		self.reversed >> level & 1 == 1
	}

	/// Whether every level has a known extent, so `total` is the domain.
	pub fn is_finite(&self) -> bool {
		self.extents[..self.depth()].iter().all(|extent| *extent != u64::MAX)
	}

	/// The lane whose levels read as `levels(i)`: the inverse of `level`.
	pub fn flat_index(&self, levels: impl Fn(usize) -> u64) -> u64 {
		let mut lane = 0;
		let mut stride = 1;
		for level in 0..self.depth() {
			let mut index = levels(level);
			if self.reversed >> level & 1 == 1 {
				index = self.extents[level] - 1 - index;
			}
			lane += index * stride;
			stride = stride.saturating_mul(self.extents[level]);
		}
		lane
	}
}

impl<'e> Dispatch<'e> {
	/// The same dispatch over another range of the same map.
	pub fn over(&self, range: std::ops::Range<u64>) -> Self {
		Self { range, ..self.clone() }
	}

	/// The same dispatch with level 0 sized where it was open.
	pub fn sized(&self, extent: u64) -> Self {
		Self {
			map: self.map.sized(extent),
			..self.clone()
		}
	}

	/// The same dispatch with level 0 replaced by `extent`.
	pub fn resized(&self, extent: u64) -> Self {
		Self {
			map: self.map.resized(extent),
			..self.clone()
		}
	}

	/// The same lanes with every index level retained again: a gathered level
	/// is a fresh level, so nothing about it is nullified yet.
	pub fn retaining_all(&self) -> Self {
		Self {
			retained: IndexLevels::all(),
			..self.clone()
		}
	}

	/// The same lanes under another footprint.
	pub fn with_footprint<'s>(&self, footprint: &'s Footprint) -> Dispatch<'s>
	where
		'e: 's,
	{
		Dispatch {
			fixed: Fixed {
				scope: self.fixed.scope,
				footprint: Some(footprint),
				varargs: self.fixed.varargs,
				position: self.fixed.position,
				outer: self.fixed.outer,
			},
			map: self.map,
			retained: self.retained,
			range: self.range.clone(),
		}
	}
}
/// One lane of a dispatch, answering the context traits from the map.
#[derive(Clone, Copy)]
pub struct Lane<'a> {
	dispatch: &'a Dispatch<'a>,
	lane: u64,
}

impl<'a> Lane<'a> {
	fn depth(&self) -> usize {
		self.dispatch.map.depth()
	}

	/// The chain value at `level`, zero where nullification cleared it.
	pub fn level(&self, level: usize) -> u64 {
		if !self.dispatch.retained.contains_level(level) {
			return 0;
		}
		let depth = self.depth();
		match level < depth {
			true => self.dispatch.map.level(self.lane, level),
			false => std::iter::successors(self.dispatch.fixed.outer, |link| link.outer).nth(level - depth).map_or(0, |link| link.index),
		}
	}

	fn chain_len(&self) -> usize {
		self.depth() + std::iter::successors(self.dispatch.fixed.outer, |link| link.outer).count()
	}

	/// The chain as nullification canonicalizes it: trailing zeroed levels
	/// carry no information and are dropped, keeping at least one level.
	fn canonical(&self) -> impl Iterator<Item = u64> + '_ {
		let len = (0..self.chain_len()).rev().find(|&level| self.level(level) != 0).map_or(1, |level| level + 1);
		(0..len).map(move |level| self.level(level))
	}

	/// Runs `body` with the serve-side context this lane stands for, its chain
	/// built on the stack for the call.
	pub fn with_serve<R>(&self, body: impl FnOnce(&ContextImpl<'_>) -> R) -> R {
		let mut levels: Vec<u64> = self.canonical().collect();
		levels.reverse();
		let fixed = &self.dispatch.fixed;
		let mut base = ContextImpl::root(fixed.scope);
		if let Some(footprint) = fixed.footprint {
			base = base.with_footprint(footprint);
		}
		if let Some(varargs) = fixed.varargs {
			base = base.with_varargs(varargs);
		}
		if let Some(position) = fixed.position {
			base = base.with_position(position);
		}
		// The canonical chain replaces the whole chain, outer part included.
		descend(&levels, None, &base, body)
	}
}

/// Builds `levels` (outermost first) as stack-linked chain links and hands
/// the innermost to `body` through the base context.
fn descend<R>(levels: &[u64], outer: Option<&IndexLink<'_>>, base: &ContextImpl<'_>, body: impl FnOnce(&ContextImpl<'_>) -> R) -> R {
	match levels.split_last() {
		None => body(base),
		Some((&innermost, rest)) if rest.is_empty() => match outer {
			Some(outer) => body(&base.promoted(outer, innermost)),
			None => body(&base.replaced(innermost)),
		},
		Some((_, _)) => {
			let (&outermost, inner) = levels.split_first().expect("non-empty");
			let link = IndexLink { index: outermost, outer };
			descend(inner, Some(&link), base, body)
		}
	}
}

impl Ctx for Lane<'_> {}

impl InjectIndex for Lane<'_> {
	fn set_index(&mut self, index: u64) {
		self.lane = index;
	}
}

impl ExtractIndices for Lane<'_> {
	fn try_index(&self) -> Option<impl Iterator<Item = usize>> {
		Some(self.canonical().map(|level| level as usize))
	}
}

impl ExtractFootprint for Lane<'_> {
	fn try_footprint(&self) -> Option<&Footprint> {
		self.dispatch.fixed.footprint
	}
}

impl ExtractRealTime for Lane<'_> {
	fn try_real_time(&self) -> Option<f64> {
		self.dispatch.fixed.scope.real_time()
	}
}

impl ExtractAnimationTime for Lane<'_> {
	fn try_animation_time(&self) -> Option<f64> {
		self.dispatch.fixed.scope.animation_time()
	}
}

impl ExtractPointerPosition for Lane<'_> {
	fn try_pointer_position(&self) -> Option<DVec2> {
		self.dispatch.fixed.scope.pointer_position()
	}
}

impl ExtractPosition for Lane<'_> {
	fn try_position(&self) -> Option<impl Iterator<Item = DVec2>> {
		self.dispatch.fixed.position.map(|head| std::iter::successors(Some(head), |link| link.outer).map(|link| link.position))
	}
}

impl ExtractVarArgs for Lane<'_> {
	fn vararg(&self, index: usize) -> Result<crate::context::DynRef<'_>, VarArgsResult> {
		crate::context::vararg_at(self.dispatch.fixed.varargs, index)
	}

	fn varargs_len(&self) -> Result<usize, VarArgsResult> {
		crate::context::varargs_len(self.dispatch.fixed.varargs)
	}

	fn hash_varargs(&self, hasher: &mut dyn Hasher) {
		crate::context::hash_vararg_chain(self.dispatch.fixed.varargs, hasher)
	}
}

impl<'a> ExtractArena for Lane<'a> {
	type ArenaRef = &'a Arena;
	fn arena(&self) -> &'a Arena {
		self.dispatch.fixed.scope.arena()
	}
}

/// The same hash as the serve-side context this lane stands for.
impl graphene_hash::CacheHash for Lane<'_> {
	fn cache_hash<H: Hasher>(&self, state: &mut H) {
		match self.dispatch.fixed.footprint {
			Some(footprint) => {
				1u8.hash(state);
				footprint.cache_hash(state);
			}
			None => 0u8.hash(state),
		}
		let mut count = 0u64;
		for level in self.canonical() {
			level.hash(state);
			count += 1;
		}
		count.hash(state);
		count = 0;
		let mut position = self.dispatch.fixed.position;
		while let Some(link) = position {
			link.position.x.to_bits().hash(state);
			link.position.y.to_bits().hash(state);
			count += 1;
			position = link.outer;
		}
		count.hash(state);
		self.hash_varargs(state);
		self.dispatch.fixed.scope.hash().hash(state);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::context::{ContextImpl, nullify_index_levels};
	use crate::registry::cache_key;

	fn keep(levels: &[u8]) -> IndexLevels {
		levels.iter().fold(IndexLevels::empty(), |mask, &level| mask.with_level(level))
	}

	/// The serve side: the chain a Repeat of `copies` over a body of `inner`
	/// lanes builds for one lane, nullified to `keep`.
	fn served_key(root: &ContextImpl<'_>, scope: &EvalScope<'_>, arena: &Arena, inner: u64, lane: u64, reverse: bool, copies: u64, keep: IndexLevels) -> u64 {
		let (copy, within) = (lane / inner, lane % inner);
		let copy = match reverse {
			true => copies - 1 - copy,
			false => copy,
		};
		let mut frame = IndexLink { index: 0, outer: None };
		let served = root.push_level(&mut frame, copy, within);
		let chain = nullify_index_levels(served.index_head(), keep, arena).unwrap();
		cache_key(&served.nullified(ContextFeatures::all(), chain, scope))
	}

	#[test]
	fn a_lane_keys_like_the_serve_chain_it_stands_for() {
		let arena = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = EvalScope::new(Some(0.5), None, None, &generations, &arena);
		let root = ContextImpl::root(&scope);
		let (inner, copies) = (4u64, 3u64);
		for reverse in [false, true] {
			for keep in [keep(&[0, 1]), keep(&[1]), keep(&[0])] {
				let dispatch = Dispatch::new(&root, LaneMap::flat(inner * copies).refined(inner, reverse).unwrap(), 0..inner * copies).nullified(keep);
				for lane in 0..inner * copies {
					let expected = served_key(&root, &scope, &arena, inner, lane, reverse, copies, keep);
					assert_eq!(cache_key(&dispatch.lane(lane)), expected, "lane {lane} reverse {reverse} keep {keep:?}");
					let via_serve = dispatch.lane(lane).with_serve(|ctx| cache_key(ctx));
					assert_eq!(via_serve, expected, "with_serve for lane {lane} reverse {reverse} keep {keep:?}");
				}
			}
		}
	}

	#[test]
	fn serve_at_builds_the_refined_chain() {
		let arena = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = EvalScope::new(None, None, None, &generations, &arena);
		let root = ContextImpl::root(&scope);
		let dispatch = Dispatch::new(&root, LaneMap::open(), 0..6).sized(6).refined(3, false).unwrap();
		for (lane, expected) in [(0u64, vec![0usize]), (1, vec![1]), (4, vec![1, 1]), (5, vec![2, 1])] {
			let served = dispatch.serve_at(lane).unwrap();
			let chain: Vec<usize> = served.try_index().unwrap().collect();
			assert_eq!(chain, expected, "lane {lane}");
			assert_eq!(served.try_index().unwrap().nth(1).unwrap_or(0), (lane / 3) as usize, "copy of lane {lane}");
		}
	}
	#[test]
	fn a_map_decomposes_and_reverses() {
		let map = LaneMap::flat(12).refined(4, true).unwrap();
		assert_eq!(map.depth(), 2);
		assert_eq!(map.total(), 12);
		assert_eq!((map.level(5, 0), map.level(5, 1)), (1, 1), "lane 5 is inner 1 of copy 1, reversed among 3 copies");
		assert_eq!(map.level(11, 1), 0, "the last lane is the last copy, which reversal makes copy 0");
		assert!(LaneMap::flat(10).refined(4, false).is_none(), "a level is a whole number of copies");
	}
}
