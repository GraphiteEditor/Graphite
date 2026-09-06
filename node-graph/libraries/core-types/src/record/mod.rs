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

pub use access::{Rec, RecordValue, apply_plan, borrow_element, erase_static, read_at, read_element, token_only, write_element, write_element_sized, write_field};
pub use frames::{FrameArena, FrameScope, Frames};
pub use input::{DerivedLazyInput, DerivedRecordEdge, ElementEdge, ElementLazyInput, LevelStatus, RecordEdgeInput, RecordExtract, RecordLazyInput, fill_frames, materialize_batch, materialize_level};
pub use layout::{
	ElToken, ElementSpec, ElementWrite, ElementWritePick, ElementWritePickHashed, ElementWritePickPlain, FieldDesc, FieldOffset, FieldWrite, InputReads, Layout, LayoutMeta, RecordLayout, copy_plan,
	element_dims, element_parked, element_write, element_write_hashed, empty_layout,
};
pub use owned::{OwnedRecord, deepen_field_value, register_deep_element_clone, register_deep_field_value, replay_field_value};
pub use promote::{Promotion, assert_promoted, register_element_promote, register_field_promote, register_retained_heap};
pub use route::{RecordSource, SourcePlan};
pub use run::{Group, GroupItem, RunBuilder, RunColumn, RunView};
pub use serve::{FrameClaim, MaterializedSpan, Served, SlotRun, serve_input};
pub use testkit::{LiftedSource, ServedRecord, capture, test_frames};
