# AI Models Roadmap

This document tracks AI model disciplines that are relevant to a graphics editing workflow and could be exposed as node graph tools within Graphite. It originated from the brainstorming discussion in issue #1694 and serves as a single reference point for categories under consideration. Nothing here commits to a specific architecture or model; it is a living list to be refined as nodes are designed and implemented.

## Status legend

- Proposed: identified as useful, not yet scoped.
- Planned: scoped and queued for implementation.
- In progress: being implemented.
- Available: usable in the editor.

## Generation and synthesis

- Text-to-image generation — Proposed
- Image-to-image translation — Proposed
- Outpainting, extending beyond the original canvas (distinct from infill) — Proposed
- Texture and pattern synthesis — Proposed

## Local and guided editing

- Inpainting / infill, filling masked regions (distinct from outpainting) — Proposed
- Prompt-guided local edits / instruct-style editing — Proposed
- Conditioned generation (edge, depth, pose, or scribble guidance) — Proposed
- Image harmonization, matching inserted content to a scene — Proposed
- Relighting — Proposed

## Restoration and enhancement

- Super-resolution / upscaling — Proposed
- Denoising — Proposed
- JPEG and web compression artifact removal — Proposed
- Face restoration — Proposed
- Text restoration / legibility recovery — Proposed
- Colorization — Proposed

## Selection and masking

- Semantic and instance segmentation — Proposed
- Background removal, as a distinct one-click UX from general segmentation — Proposed
- Matting and edge refinement, alpha matting beyond chroma keying — Proposed
- Object detection — Proposed

## Analysis and understanding

- Depth estimation — Proposed
- Pose estimation — Proposed
- Image captioning and tagging — Proposed
- OCR / text recognition — Proposed

## Conversion and format

- Vectorization / raster-to-vector tracing — Proposed
- Style transfer — Proposed

## Video and temporal

- Video super-resolution with temporal coherence — Proposed
- Frame interpolation — Proposed
- Temporally consistent editing and propagation — Proposed

## Notes

These categories are intentionally broad. Some overlap (for example, background removal builds on segmentation and matting) but are listed separately because they map to distinct user-facing tools. Additions and refinements are welcome; see issue #1694 for the original discussion.
