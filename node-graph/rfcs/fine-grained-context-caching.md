# Summary

Add a new compilation pass to "nullify" parts of the dynamic `Context` based on the usage within the graph to avoid unnecessary cache invalidations.

# Motivation

Caching of node outputs can only be done if the input (`Context`) the node was evaluated with has not changed between subsequent evaluations. This can lead to "false invalidation" which is when the cache is invalidated even though the node did not even depend on value that changed and still returns the same result.

```rs
// This node does not use any time information so we don't need to rerun it if the time has changed.
#[node_macro]
fn use_footprint(ctx: impl Ctx + ExtractFootprint, a: u32) -> {...}
```

To mitigate this, we introduced a relatively fine grained `Extract*` API for interacting with the context. We can use the trait annotations produced by this system to infer which parts of the context are used on which graph evaluation paths during graph compile time.

# Guide-level explanation

Our current implementation of the `OwnedContextImpl` struct contains many values which can be used to pass data to nodes. But most of the time, the majority of these fields will be unused by the nodes, but when considering the equality of two `OwnedContextImpl` instances, they have to be considered.

```rs
pub struct OwnedContextImpl {
	footprint: Option<crate::transform::Footprint>,
	varargs: Option<Arc<[DynBox]>>,
	parent: Option<Arc<dyn ExtractVarArgs + Sync + Send>>,
	index: Option<usize>,
	real_time: Option<f64>,
	animation_time: Option<f64>,
}
```

## Why is `Context` equality important?

In Graphene, every node has to be *idempotent* that means that when we provide it with the same input, it will return the same output. This is a really useful property for caching because we can effectively only do the computation once and then reuse the result which could be significantly cheaper.

What is the input then?

The input to all of nodes is of type `Context` which in of itself is just defined as:

```rs
pub type Context = Option<Arc<OwnedContextImpl>>;
```

We use this unified dynamic context type because this means we only have to compile one version of a node and all nodes are compatible with each other but this is not a formal limitation (and should never be considered to be a given).

The different parts of the context (e.g. `Footprint`, `index`, ...) are called *features*.

It thus makes sense that we have to check the equality of `Context` objects to test if we can reuse a cached value or not. If as in the example above a node only relies on one part of the `Context` we don't really care if some other part has changed and the contexts can be considered equal for use in **this node**.

Cache nodes compare the equality of inputs based on the hash code. To stay compatible with the existing API, we can "zero out" parts of the `OwnedContextImpl` by setting unused variants to `None`. This is done by a context modification node which is placed into the graph by the compiler.

The `ExtractAll` trait can be used to create a new Context based on the previous one which can be utilized by nodes which need to modify the context for their child nodes but don't depend on the data themselves.

```rs
#[node_macro::node(category(""))]
async fn transform(ctx: impl Ctx + ExtractAll, ...) {...}
 ```

## Context Feature Injection

Some nodes need to provide context features for their downstream dependencies (in the function call stack building phase). This is accomplished through `Inject*` traits that complement the `Extract*` traits:

```rs
// A node that injects index information for downstream map operations
#[node_macro::node(category("Iteration"))]
fn map_with_index<T>(
    ctx: impl Ctx + InjectIndex,
    collection: Vec<T>,
    mapper: impl Node<T, Output = U>,
) -> Vec<U> {
    collection.iter().enumerate().map(|(index, item)| {
        // This node injects the current index into the context
        // for the mapper node to extract via ExtractIndex
        let ctx_with_index = ctx.with_injected_index(index);
        mapper.eval_with_context(ctx_with_index, item)
    }).collect()
}

// Downstream nodes can extract the injected index
#[node_macro::node(category("Utility"))]
fn use_index(ctx: impl Ctx + ExtractIndex, value: f64) -> f64 {
    let index = ctx.index().unwrap_or(0);
    value * (index as f64)
}
```

### Injection Hierarchy and Precedence

When a node both extracts and injects the same feature:
- **Extract-then-Inject**: Node extracts from upstream, processes, then injects modified version downstream
- **Inject-Override**: Injected features take precedence over upstream extracted features
- **Injection Scope**: Injected features are only available to immediate downstream nodes in the evaluation chain

# Reference-level explanation

The different `Extract*` and `Inject*` traits are exported by the node macro and are included as part of the document node definition to inform the compiler about features extracted and injected by every node. Note that the `ExtractAll` will be ignored in this analysis.

## Context Nullification Analysis

The compiler determines where to insert context nullification nodes through branch analysis:

1. **Extract Requirement Tracking**: For each branch in the graph, track the extract requirements all the way back to their corresponding inject nodes. Every extracted feature must have a corresponding inject node somewhere upstream, otherwise this is a compile error.
2. **Branch Convergence Analysis**: When two branches with different extract requirements meet (at a node that takes multiple inputs), one or both branches can have their context nullified to remove features only needed in the other branch.
3. **Post-Injection Nullification**: After an inject node, the extract needs of downstream nodes are satisfied for that inject type. At this point we can check if all the features that the inject node provides are actually used downstream, and if not, nullify them immediately.
4. **Injection Scope Optimization**: After every inject node, analyze whether all injected features are actually consumed by downstream nodes. Unused injected features can be nullified right at the injection point.

## Inject* Trait System

The injection system provides these complementary marker traits to Extract*:

```rs
pub trait InjectFootprint {}
pub trait InjectRealTime {}
pub trait InjectIndex {}
pub trait InjectVarArgs {}
```

## Context Feature Modification Traits

The modification system provides marker traits for nodes that transform context features without necessarily depending on them:

```rs
pub trait ModifyFootprint: ExtractFootprint + InjectFootprint {}
pub trait ModifyRealTime: ExtractRealTime + InjectRealTime {}
pub trait ModifyIndex: ExtractIndex + InjectIndex {}
pub trait ModifyVarArgs: ExtractVarArgs + InjectVarArgs {}
```

### Conditional Context Dependencies

Modify* traits represent a special case in context analysis:

```rs
// Transform node example - modifies footprint but doesn't need it unless downstream requires it
#[node_macro::node(category("Transform"))]
fn transform(
    ctx: impl Ctx + ModifyFootprint,
    input: Vector,
    transform: Transform2D,
) -> Vector {
    // This node can extract the footprint, modify it, and inject the result
    // But if no downstream node needs the footprint, this node doesn't need it either
    let modified_footprint = ctx.footprint().transform(transform);
    // ... transform logic ...
}
```

### Optimization Implications for Modify* Traits

1. **Conditional Requirements**: Modify* nodes only require their features if downstream nodes extract them
2. **Pass-through Optimization**: If no downstream extraction occurs, the Modify* node can be treated as if it has no context requirements
3. **Transform Chains**: Multiple Modify* nodes can be chained together, with requirements only propagating if there's a final Extract* consumer

Example optimization:

```
[Node A] -> [ModifyFootprint] -> [ModifyFootprint] -> [ExtractRealTime]
              ↑                    ↑                    ↑
        No footprint needed   No footprint needed   Only real time needed
        
[Node A] -> [ModifyFootprint] -> [ModifyFootprint] -> [ExtractFootprint]
              ↑                    ↑                    ↑
        Footprint needed     Footprint needed     Footprint needed
```

This allows transform chains to be optimized when their modifications aren't actually consumed downstream.

Note that "downstream" in this context refers to nodes that are called later in the function call stack building phase, which is inverted compared to the usual data flow direction.

This can be implemented as a compiler pass similar to the compose node insertion.

### Error Handling

- Compile-time validation: Every Extract* must have corresponding Inject* upstream

# Drawbacks

Having an extra compiler pass will impact the performance slightly although the impact is expected to be small because we already have a backlink structure and a topological sort of proto nodes which we can repurpose.

# Rationale and alternatives

Moving this fine grained cache invalidation to a compiler pass allows us to implement this with a minimal impact on the graph runtime. Other options would consist of tracking the usage of features at graph runtime inducing overheads.

This is expected to have the biggest impact on real time applications such as animation or when working with non-footprint aware nodes which would also benefit from this optimization.

# Unresolved questions

- ~~How do we communicate to the context modification nodes which parts of the context should be "zeroed"?~~
- ~~How does this interact with "smart caching" (nodes which use e.g. the Footprint to approximate the result through upscaling)?~~

# Future possibilities

- ~~Adding `Inject*` annotation to complement the `Extract*` ones to provide even more fine grained control over caching.~~
