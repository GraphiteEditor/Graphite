//! The packed-record tier at rank 0. A record is the element at offset 0
//! plus one field per written attribute; its [`Layout`] is computed at
//! wiring from the upstream write set and never serialized. Records of
//! inline layouts live in the [`RecordValue`] itself; larger ones live as
//! per-lane views on the evaluation's [`Frames`], which the root owns and
//! every node claims its own frame out of. Kernels route them as opaque
//! [`RecordValue`]s that carry
//! their provenance. Only generated or wiring code touches offsets, so a
//! safe kernel cannot misalign a field.

mod access;
mod frames;
mod input;
mod layout;
mod owned;
mod promote;
mod route;
mod run;
mod serve;
#[cfg(test)]
mod test_support;
mod testkit;

pub use access::{Rec, RecordValue, apply_plan, borrow_element, erase_static, read_at, read_at_defaulting, read_element, token_only, write_element, write_element_sized, write_field};
pub use frames::{FrameArena, FrameScope, Frames};
pub use input::{
	DerivedLazyInput, DerivedRecordInput, ElementInput, ElementLazyInput, LevelStatus, RecordExtract, RecordInput, RecordLazyInput, fill_dispatch, fill_frames, forward_batch, forward_dispatch,
	inner_extent_of, materialize_batch, materialize_dispatch, materialize_level,
};
#[cfg(debug_assertions)]
pub use input::{note_batch_consumer, note_kernel_batch, take_batch_tally, take_kernel_tally};
pub use input::{note_render_nanos, take_render_nanos};
pub use layout::{
	ElToken, ElementSpec, ElementWrite, ElementWritePick, ElementWritePickHashed, ElementWritePickPlain, FieldDesc, FieldOffset, FieldWrite, InputReads, Layout, LayoutMeta, NamedRead, NamedWrite,
	RecordLayout, copy_plan, element_dims, element_parked, element_write, element_write_hashed, empty_layout,
};
pub use owned::{OwnedRecord, deepen_field_value, has_deep_element_glue, register_deep_element_clone, register_deep_field_value, replay_field_value};
pub use promote::{Promotion, assert_promoted, register_element_promote, register_field_promote, register_retained_heap};
pub use route::{RecordSource, SourcePlan};
pub use run::{Group, GroupItem, RunBuilder, RunColumn, RunView, run_to_owned_list};
pub use serve::{FrameClaim, LaneSpan, MaterializedSpan, Served, SlotRun, serve_input};
pub use testkit::{LiftedSource, ServedRecord, capture, fixtures as test_fixtures, test_frames};
