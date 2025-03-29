- Feature Name: fine_grained_context_caching
- Start Date: 2025-03-29
- RFC PR: [rust-lang/rfcs#0000](https://github.com/GraphiteEditor/Graphite/pull/2500)

# Summary
[summary]: #summary

Add a new compilation pass to "zero out" parts of the dynamic `Context` based on the usage within the graph to avoid unnecessary cache invalidations.

# Motivation
[motivation]: #motivation

Caching of node outputs can only be done if the input (`Context`) the node was evaluated with has not changed between subsequent evaluations. This can lead to "false invalidation" which is when the cache is invalidated even though the node did not even depend on value that changed and still returns the same result.

```rust
// This node does not use any time information so we don't need to rerun it if the time has changed.
#[node_macro]
fn use_footprint(ctx: impl Ctx + ExtractFootprint, a: u32) -> {...}
```
 To mitigate this, we introduced a relatively fine grained `Extract*` API for interacting with the context. We can use the trait annotations produced by this system to infer which parts of the context are used on which graph evaluation paths during graph compile time.

# Guide-level explanation
[guide-level-explanation]: #guide-level-explanation

Our current implementation of the `OwnedContextImpl` struct contains many values which can be used to pass data to nodes. But most of the time, the majority of these fields will be unused by the nodes, but when considering the equality of two `OwnedContextImpl` instances, they have to be considered.

```rust
pub struct OwnedContextImpl {
	footprint: Option<crate::transform::Footprint>,
	varargs: Option<Arc<[DynBox]>>,
	parent: Option<Arc<dyn ExtractVarArgs + Sync + Send>>,
	index: Option<usize>,
	system_time: Option<f64>,
	animation_time: Option<f64>,
}
```

## Why is `Context` equality important?
In Graphene, every node has to be *idempotent* that means that when we provide it with the same input, it will return the same output. This is a really useful property for caching because we can effectively only do the computation once and then reuse the result which could be significantly cheaper.
What is the input then?
The input to all of nodes is of type `Context` which in of itself is just defined as:
```rust
pub type Context = Option<Arc<OwnedContextImpl>>;
```

We use this unified dynamic context type because this means we only have to compile one version of a node and all nodes are compatible with each other but this is not a formal limitation (and should never be considered to be a given).
The different parts of the context (e.g. `Footprint`, `index`, ...) are called *features*.
It thus makes sense that we have to check the equality of `Context` objects to test if we can reuse a cached value or not. If as in the example above a node only relies on one part of the `Context` we don't really care if some other part has changed and the contexts can be considered equal for use in **this node**.
Cache nodes compare the equality of inputs based on the hash code. To stay compatible with the existing API, we can "zero out" parts of the `OwnedContextImpl` by setting unused variants to `None`. This is done by a context modification node which is placed into the graph by the compiler.
The `ExtractAll` trait can be used to create a new Context based on the previous one which can be utilized by nodes which need to modify the context for their child nodes but don't depend on the data themselves.

```rust
#[node_macro::node(category(""))]
async fn transform(ctx: impl Ctx + ExtractAll, ...)  {...}
 ```

# Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

The different `Extract*` traits are exported by the node macro and could thus be included as part of the document node definition to inform the compiler about features extracted in every node. Note that the `ExtractAll` will be ignored in this analysis. Any usages of partial context data are propagated downstream and all nodes are identified in which the number of extracted features changes between the upstream and downstream. At these locations a context modification needs to be inserted which "zeros" the data no longer used in the upstream part of the graph.
Note that the number of features extracted can usually only increase except for cases where a node decides to inject data into the call chain.
This will be the case when building lambda expressions, the node driving the lambda evaluation (e.g. a map or a fold node) would insert data such as the index into the call chain.
We might consider adding a special `Inject*` annotation in the future to indicate that the downstream of this node does not need to provide the feature even though the upstream does need it.
This can be implemented as a compiler pass similar to the compose node insertion.

# Drawbacks
[drawbacks]: #drawbacks

Having an extra compiler pass will impact the performance slightly although the impact is expected to be small because we already have a backlink structure and a topological sort of proto nodes which we can repurpose.

# Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

Moving this fine grained cache invalidation to a compiler pass allows us to implement this with a minimal impact on the graph runtime. Other options would consist of tracking the usage of features at graph runtime inducing overheads.
This is expected to have the biggest impact on real time applications such as animation or when working with non-footprint aware nodes which would also benefit from this optimization.

# Unresolved questions
[unresolved-questions]: #unresolved-questions

- How do we communicate to the context modification nodes which parts of the context should be "zeroed"?
- How does this interact with "smart caching" (nodes which use e.g. the Footprint to approximate the result through upscaling)?


# Future possibilities
[future-possibilities]: #future-possibilities

Adding `Inject*` annotation to complement the `Extract*` ones to provide even more fine grained control over caching.
