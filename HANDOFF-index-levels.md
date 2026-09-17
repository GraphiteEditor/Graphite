# Index-level narrowing: investigation state

Branch `brick-wave-investigation`. Why `~/Downloads/brick-waves.graphite` gets no caching around its
palette lookup, what we changed, and where it still breaks.

## The original problem

`read_index` takes its *Loop Level* as a runtime input, so its signature can only declare
`ExtractIndices` → `ContextFeature::ExtractIndex(u8::MAX)` → `IndexLevels::all()`.

A saturated mask is absorbing in both places it is consumed:

- `nullify_index_levels` (`core-types/src/context.rs:283`) early-returns on `is_all()`, so nothing is
  nullified
- `IndexLevels::lifted` (`context.rs:253`) has `u32::MAX => self`, so propagation never narrows it
- `ContextModification::contains` therefore always succeeds, so `we_introduce_new_deps` is false and
  **no cache boundary is ever inserted anywhere in the cone**

In `brick-waves` the whole palette chain (`StringValue → split → item_at_index → join → split →
item_at_index → join → hex_to_color`) sits under this, and re-derives per lane: 36864 evaluations
where 96 would do.

## What we built (all in the preprocessor, none of it node-specific in the compiler)

`node-graph/preprocessor/src/lib.rs`:

1. `restore_declared_context_features` — re-declares `extract`/`inject`/`index_levels` from the
   registry. A document serializes `extract`/`inject` (so it can carry a declaration from an older
   build) and `index_levels` is `#[serde(skip)]`, so the field was only ever trustworthy by accident.
2. `determine_index_reads` — when `loop_level` is a literal, narrows `index_levels` to
   `{loop_level + 1}` (the kernel reads `nth(loop_level + 1)`).
3. `expand_network` — carries the wrapper's declaration onto the substituted proto node. Needed
   because a substitution replaces `implementation` with a template network, and the declaration
   would otherwise stay on the wrapper that `flatten` dissolves.

**Verified neutral**: with narrowing disabled (`LEVEL_MODE=off`) the render is byte-identical to
baseline (`md5 87fd5f79b006`). So 1 and 3 are sound on their own.

## What narrowing achieves, and how it breaks

Masks stop being saturated: precise `{1,2,3,4,6,7}` appear, boundaries go 82 → 109.

`read_index` itself is **correct** under narrowing — verified at runtime, not inferred:

| mode | chain seen | value | distinct chains |
|---|---|---|---|
| `LEVEL_MODE=only0` | `[0, 0, N, 0]` depth 4 | `nth(2) = N` | 36864 |
| narrowed | `[0, 0, N]` depth 3 | `nth(2) = N` | **96** |

Both return the full range 0..95. Nullification strips only the trailing zeroed level and the read
still lands on `N`. That is the 384× win we were after.

But `hex_to_color` receives **1 distinct hex string instead of 63**, and the render collapses from 65
colors to 3. So the correct values are lost *downstream* of `read_index`, in the string chain.

## Ruled out

- **Off-by-one in the declared level.** Declaring 2, 3, or 4 for the `loop_level=1` reader gives
  byte-identical broken output. The mask *value* is irrelevant; only its non-saturation matters.
- **Keeping level 0 as well.** No change.
- **`InjectIndex` falsely discharging the requirement.** `ContextFeature::InjectIndex → INDEX` exists
  (`context.rs:534`) but the macro never emits it (`parsing.rs:1041`), so `inject` never contains
  `INDEX`. `new_deps &= !inject` never clears it and `we_supply_existing_deps` never fires for it.
  The feature is never falsely discharged — only the *level* arithmetic discharges.
- **A node overstating `pushed_levels`.** `PROBE_LEVELS` dumps every lift. The only requirement
  deaths are legitimate lane-drives (`ItemAtIndex` on `list`, `Fill` on its color inputs, …). The two
  `Repeat`s consume exactly the two levels `read_index` declares: `{0,1,2} → {0,1} → {0}`. The
  boundary mask follows correctly from the declarations.
- **The split/item/join level arithmetic alone.** See the repro below — it does not collapse.

Narrowing only the `loop_level=0` reader (`LEVEL_MODE=only0`) is byte-identical to baseline and adds
4 boundaries, but measures the same as `off` (10s vs 10s over 8 repeats, debug). No partial win to
bank; the payoff needs the outer-loop reader.

## Minimal reproduction (does NOT yet reproduce)

`proto::test::nested_index_lookup_boundary_levels` in `graph-craft/src/proto.rs`, with helpers
`nested_index_lookup_network` and `index_level_boundaries`.

Builds `split → item_at_index → join` twice against two resolved-level `read_index` nodes, plus two
more level-consuming hops standing in for the `Repeat`s. Masks come out `0, 2, 1, 0, 1, 0` — every
boundary keeps a level its subtree reads. **No collapse.**

It is committed as a *characterization* test, not a correctness assertion: an earlier assertion
("a boundary requiring INDEX must retain some level") passed for the wrong reason, because the real
failure is mask `{0}` above content that varies over deeper levels, not an empty mask. That invariant
can't be written until the intended semantics are settled.

## Leading hypothesis

The missing ingredient is the real `Repeat`. `string_join` only *consumes* a level; `Repeat` also
**injects an index at runtime**. That asymmetry is where the model's two jobs diverge:
`ContextModification` computes a cache key *and* rewrites the chain the subtree receives. "A `Repeat`
supplies this level, so the requirement vanishes above it" is sound for the **key** — nothing above
the `Repeat` varies over it — but the subtree below still evaluates against whatever chain survives.

`repeat-nodes` is not a `graph-craft` dependency, so a repro including it belongs in
`interpreted-executor`, asserting on evaluated values rather than masks.

## Env gates (all inert when unset)

| var | where | effect |
|---|---|---|
| `LEVEL_MODE=off` | preprocessor | disable narrowing → byte-identical to baseline |
| `LEVEL_MODE=only0` / `only1` | preprocessor | narrow only that `loop_level` |
| `LEVEL_BUMP=N` | preprocessor | add N to declared level for `loop_level >= 1` |
| `PROBE_LEVELS` | `proto.rs` | dump every `lifted` call that changes a mask |
| `PROBE_READ_INDEX` | `gcore/src/context.rs` | dump each newly seen chain + value |
| `PROBE_HEX` | `math/src/lib.rs` | dump distinct `hex_to_color` inputs |

**Narrowing is ON by default, so the tree renders wrong.** Use `LEVEL_MODE=only0` for a correct run.
Remove the probes and the `LEVEL_MODE`/`LEVEL_BUMP` block before this is fit to merge.

## Artwork

Committed under `test-artwork/`:

- `brick-waves-v3.graphite` — the migrated working copy. **Every measurement in this document used
  this file**; it is the one known to reproduce.
- `brick-waves.graphite` — the original as saved from the editor. Kept so nothing is lost, but it
  **does not load on this branch**:

  ```
  GraphError { identifier: "graphic_nodes::graphic::ToGraphicNode", error: "No implementations found" }
  ```

  It predates the rebase, so it needs a migration that does not exist here yet. Use the v3 copy.

## Repro commands

```sh
cargo build -p graphene-cli
ART=test-artwork/brick-waves-v3.graphite

# correct vs broken
LEVEL_MODE=only0 ./target/debug/graphene-cli export $ART -o ok.svg    # 65 colors
                 ./target/debug/graphene-cli export $ART -o bad.svg   # 3 colors
grep -o 'fill="#[0-9a-f]*"' bad.svg | sort -u | wc -l

# where distinctness is lost
PROBE_READ_INDEX=1 ./target/debug/graphene-cli export $ART -o p.svg   # read_index: correct, 96 values
PROBE_HEX=1        ./target/debug/graphene-cli export $ART -o h.svg   # hex_to_color: 1 input, should be 63

# compile-time masks (note: -o /dev/null short-circuits evaluation, use a real file)
./target/debug/graphene-cli compile -p $ART | grep -o "index_levels: IndexLevels([0-9]*)" | sort | uniq -c
```
