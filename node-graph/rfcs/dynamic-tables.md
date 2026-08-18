- Feature Name: dynamic_tables
- Start Date: 2025-04-06
- RFC PR: [Graphite/#0000](https://github.com/rust-lang/rfcs/pull/0000)
- Rust Issue: [Graphite/rust#0000](https://github.com/rust-lang/rust/issues/0000)

# Summary
[summary]: #summary

Unify Tables to use one datatype with support for adding named collums and adding new values at runtime with support for structural sharing

# Motivation
[motivation]: #motivation

Spreadsheets are a way in which users can look at a visual representation of the data flowing through the graph and attach new data to the flow. This can either happen as adding or removing rows, or as adding or removing entire colums.

# Guide-level explanation
[guide-level-explanation]: #guide-level-explanation


Attributes are the colums of our tables. Each attribute is only of one specific type, this allows us to only store the type information once for user defined types, allowing us to store them in line and remove the type information needed 
```rust
#[derive(Clone)]
pub enum Attribute {
    String(Vec<String>),
    U32(Vec<u32>),
    Segment(Vec<Segment>),
    Region(Vec<Region>),
    Custom(DynAttribute),
}
```

```rust
pub struct DynAttribute {
    type_id: TypeId,
    store: Vec<SmallBox<4>>,
}
```

A table row is mostly identical to a table, but must not contain more than one row.
```rust
pub struct TableRow<T> {
    primary: T,
    // Each table type will only contain one row
    attributes: HashMap<String, Attribute>,
}
```

The main datatype for everything to interact with. Each table has a signature primary data type (such as Points, Segments, etc.) and then any number of extra colums.
```rust
#[derive(Clone)]
pub struct Table<T> {
    // primary_name: String, ?
    primary: Vec<T>,
    attributes: HashMap<String, Arc<Attribute>>,
}
```

The lazy Table allows the use of structural sharing to make appending new rows to a table an O(1) operation without needing to perform a copy to keep the old version of the table around. 

```rust
#[derive(Clone)]
pub struct LazyTable<T> {
    previous: Option<Arc<LazyTable<T>>>,
    start_index: usize,
    current: Arc<Table<T>>,
}
```

Examples:

```rust
pub fn change_blend_mode_group(
    mut instance: TableRow<VectorData>,
    blend_mode: BlendMode,
) -> TableRow<VectorData> {
    instance.get_mut("blend_mode")[0] = blend_mode;
    instance
}

pub fn change_fill(
    mut instance: TableRow<Table<VectorData>>,
    fill: String,
) -> TableRow<Table<VectorData>> {
    instance.primary.primary[0] = fill;
    instance
}

pub fn merge<T: 'static + Clone, U>(table: LazyTable<T>, value: TableRow<Table<U>>) -> LazyTable<T>
where
    Table<U>: Into<T>,
{
    // todo optimize
    let mut table = table.flatten();

    table.append_scalar(value);
    table.into()
}

pub fn vector_data_table(
    points: LazyTable<DVec2>,
    segments: LazyTable<Segment>,
    regions: LazyTable<Region>,
) -> LazyTable<DVec2> {
    let mut attributes = HashMap::new();
    attributes.insert(
        "segments".into(),
        Arc::new(Attribute::Segment(segments.flatten().primary)),
    );
    attributes.insert(
        "regions".into(),
        Arc::new(Attribute::Region(regions.flatten().primary)),
    );
    Table {
        primary: points.flatten().primary,
        attributes,
    }
    .into()
}
```


Explain the proposal as if it was already included in the language and you were teaching it to another Rust programmer. That generally means:

- Introducing new named concepts.
- Explaining the feature largely in terms of examples.
- Explaining how Rust programmers should *think* about the feature, and how it should impact the way they use Rust. It should explain the impact as concretely as possible.
- If applicable, provide sample error messages, deprecation warnings, or migration guidance.
- If applicable, describe the differences between teaching this to existing Rust programmers and new Rust programmers.
- Discuss how this impacts the ability to read, understand, and maintain Rust code. Code is read and modified far more often than written; will the proposed feature make code easier to maintain?

For implementation-oriented RFCs (e.g. for compiler internals), this section should focus on how compiler contributors should think about the change, and give examples of its concrete impact. For policy RFCs, this section should provide an example-driven introduction to the policy, and explain its impact in concrete terms.

# Reference-level explanation
[reference-level-explanation]: #reference-level-explanation

This is the technical portion of the RFC. Explain the design in sufficient detail that:

- Its interaction with other features is clear.
- It is reasonably clear how the feature would be implemented.
- Corner cases are dissected by example.

The section should return to the examples given in the previous section, and explain more fully how the detailed proposal makes those examples work.

# Drawbacks
[drawbacks]: #drawbacks

Why should we *not* do this?

# Rationale and alternatives
[rationale-and-alternatives]: #rationale-and-alternatives

- Why is this design the best in the space of possible designs?
- What other designs have been considered and what is the rationale for not choosing them?
- What is the impact of not doing this?
- If this is a language proposal, could this be done in a library or macro instead? Does the proposed change make Rust code easier or harder to read, understand, and maintain?

# Prior art
[prior-art]: #prior-art

Discuss prior art, both the good and the bad, in relation to this proposal.
A few examples of what this can include are:

- For language, library, cargo, tools, and compiler proposals: Does this feature exist in other programming languages and what experience have their community had?
- For community proposals: Is this done by some other community and what were their experiences with it?
- For other teams: What lessons can we learn from what other communities have done here?
- Papers: Are there any published papers or great posts that discuss this? If you have some relevant papers to refer to, this can serve as a more detailed theoretical background.

This section is intended to encourage you as an author to think about the lessons from other languages, provide readers of your RFC with a fuller picture.
If there is no prior art, that is fine - your ideas are interesting to us whether they are brand new or if it is an adaptation from other languages.

Note that while precedent set by other languages is some motivation, it does not on its own motivate an RFC.
Please also take into consideration that rust sometimes intentionally diverges from common language features.

# Unresolved questions
[unresolved-questions]: #unresolved-questions

- What parts of the design do you expect to resolve through the RFC process before this gets merged?
- What parts of the design do you expect to resolve through the implementation of this feature before stabilization?
- What related issues do you consider out of scope for this RFC that could be addressed in the future independently of the solution that comes out of this RFC?

# Future possibilities
[future-possibilities]: #future-possibilities

Think about what the natural extension and evolution of your proposal would
be and how it would affect the language and project as a whole in a holistic
way. Try to use this section as a tool to more fully consider all possible
interactions with the project and language in your proposal.
Also consider how this all fits into the roadmap for the project
and of the relevant sub-team.

This is also a good place to "dump ideas", if they are out of scope for the
RFC you are writing but otherwise related.

If you have tried and cannot think of any future possibilities,
you may simply state that you cannot think of anything.

Note that having something written down in the future-possibilities section
is not a reason to accept the current or a future RFC; such notes should be
in the section on motivation or rationale in this or subsequent RFCs.
The section merely provides additional information.


# Implementation prototype
```rust
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

pub struct Transform;
pub struct BlendMode;
pub type VectorData = String;
type DVec2 = ();
type Segment = ();
type Region = ();

// T = DynamicTable<VectorData>
pub struct TableRow<T> {
    primary: T,
    // Each table type will only contain one row
    attributes: HashMap<String, Attribute>,
}

impl<U> TableRow<U> {
    pub fn get_mut<T: 'static>(&mut self, name: &str) -> &mut Vec<T> {
        let column = self.attributes.get_mut(name).unwrap();
        column.downcast_mut().unwrap()
    }
}


#[derive(Clone)]
pub enum Attribute {
    String(Vec<String>),
    U32(Vec<u32>),
    Segment(Vec<Segment>),
    Region(Vec<Region>),
    //Custom(DynAttribute),
}

impl Attribute {
    pub fn len(&self) -> usize {
        match self {
            Attribute::String(items) => items.len(),
            Attribute::U32(items) => items.len(),
            _ => todo!(),
        }
    }
    pub fn is_empty(&self) -> bool {
        match self {
            Attribute::String(items) => items.is_empty(),
            Attribute::U32(items) => items.is_empty(),
            _ => todo!(),
        }
    }
    pub fn push(&mut self, value: Option<&Attribute>) {
        match (self, value) {
            (Attribute::String(items), None) => items.push(Default::default()),
            (Attribute::String(items), Some(Attribute::String(value))) => {
                items.push(value[0].clone())
            }
            (Attribute::U32(items), None) => items.push(Default::default()),
            (Attribute::U32(items), Some(Attribute::U32(value))) => items.push(value[0]),
            _ => unreachable!(),
        }
    }
    pub fn append(&mut self, value: &mut Attribute) {
        match (self, value) {
            (Attribute::String(items), Attribute::String(value)) => items.append(value),
            (Attribute::U32(items), Attribute::U32(value)) => items.append(value),
            _ => unreachable!(),
        }
    }
    fn our_type_id(&mut self) -> TypeId {
        match self {
            Attribute::String(_) => TypeId::of::<String>(),
            Attribute::U32(_) => TypeId::of::<u32>(),
            _ => todo!(),
        }
    }
    pub fn downcast_mut<T: 'static>(&mut self) -> Option<&mut Vec<T>> {
        if self.our_type_id() != TypeId::of::<T>() {
            return None;
        }
        match self {
            Attribute::String(items) => Some(unsafe { &mut *(items.as_mut_ptr() as *mut Vec<T>) }),
            Attribute::U32(items) => Some(unsafe { &mut *(items.as_mut_ptr() as *mut Vec<T>) }),
            _ => todo!(),
        }
    }
}

#[derive(Clone)]
pub struct Table<T> {
    // primary_name: String,
    primary: Vec<T>,
    // Box<Vec<U>>
    attributes: HashMap<String, Arc<Attribute>>,
}

impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            primary: Default::default(),
            attributes: Default::default(),
        }
    }
}
#[derive(Clone, Default)]
pub struct LazyTable<T> {
    previous: Option<Arc<LazyTable<T>>>,
    start_index: usize,
    current: Arc<Table<T>>,
}


impl<T: Clone + 'static> LazyTable<T> {
    pub fn flatten(&self) -> Table<T> {
        let Some(previous) = &self.previous else {
            return Table::clone(self.current.as_ref());
        };
        let mut previous = previous.flatten();
        // todo fix
        previous.append(Table::clone(self.current.as_ref()));
        previous
    }
    pub fn len(&self) -> usize {
        self.start_index + self.current.primary.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn append_lazy(self: Arc<Self>, table: Table<T>) -> LazyTable<T>
    where
        T: Clone,
    {
        let mut new_attributes = HashMap::new();
        let start_index = self.len();
        for name in self.current.attributes.keys() {
            let value = table.attributes.get(name).unwrap().clone();

            new_attributes.insert(name.clone(), value);
        }
        LazyTable {
            previous: Some(self),
            start_index,
            current: Arc::new(Table {
                primary: table.primary,
                attributes: new_attributes,
            }),
        }
    }
}

impl<T> From<Table<T>> for LazyTable<T> {
    fn from(value: Table<T>) -> Self {
        LazyTable {
            current: Arc::new(value),
            ..Default::default()
        }
    }
}

impl<T: 'static> Table<T> {
    pub fn get_column_mut(&mut self, name: &str) -> &mut Vec<T> {
        let column = self.attributes.get_mut(name).unwrap();
        Arc::make_mut(column).downcast_mut().unwrap()
    }
    // TODO: optimize
    pub fn append_scalar<U>(&mut self, scalar: TableRow<Table<U>>)
    where
        Table<U>: Into<T>,
    {
        self.primary.push(scalar.primary.into());
        for (name, value) in self.attributes.iter_mut() {
            let old_value = scalar.attributes.get(name);
            Arc::make_mut(value).push(old_value);
        }
    }
    pub fn append(&mut self, mut table: Table<T>) {
        self.primary.append(&mut table.primary);
        for (name, value) in self.attributes.iter_mut() {
            let mut old_value = table.attributes.get(name).unwrap().clone();

            let old_value = Arc::make_mut(&mut old_value);
            Arc::make_mut(value).append(old_value);
        }
    }
    pub fn append_lazy(self: Arc<Self>, table: Table<T>) -> LazyTable<T>
    where
        T: Clone,
    {
        let mut new_attributes = HashMap::new();
        for name in self.attributes.keys() {
            let value = table.attributes.get(name).unwrap().clone();

            new_attributes.insert(name.clone(), value);
        }
        LazyTable {
            previous: Some(Arc::new(Table::clone(self.as_ref()).into())),
            start_index: self.primary.len(),
            current: Arc::new(Table {
                primary: table.primary,
                attributes: new_attributes,
            }),
        }
    }
}

impl<T> Table<T> {
    fn from_one(t: T) -> Self {
        Table {
            primary: vec![t],
            attributes: Default::default(),
        }
    }
}

pub fn change_blend_mode_group(
    mut instance: TableRow<VectorData>,
    blend_mode: BlendMode,
) -> TableRow<VectorData> {
    instance.get_mut("blend_mode")[0] = blend_mode;
    instance
}

pub fn change_fill(
    mut instance: TableRow<Table<VectorData>>,
    fill: String,
) -> TableRow<Table<VectorData>> {
    instance.primary.primary[0] = fill;
    instance
}

pub fn merge<T: 'static + Clone, U>(table: LazyTable<T>, value: TableRow<Table<U>>) -> LazyTable<T>
where
    Table<U>: Into<T>,
{
    // todo optimize
    let mut table = table.flatten();

    table.append_scalar(value);
    table.into()
}

pub fn vector_data_table(
    points: LazyTable<DVec2>,
    segments: LazyTable<Segment>,
    regions: LazyTable<Region>,
) -> LazyTable<DVec2> {
    let mut attributes = HashMap::new();
    attributes.insert(
        "segments".into(),
        Arc::new(Attribute::Segment(segments.flatten().primary)),
    );
    attributes.insert(
        "regions".into(),
        Arc::new(Attribute::Region(regions.flatten().primary)),
    );
    Table {
        primary: points.flatten().primary,
        attributes,
    }
    .into()
}
```
