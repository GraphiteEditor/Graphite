pub mod value;
pub use graphene_core::{generic, ops /*, structural*/};

#[cfg(feature = "caching")]
pub mod cache;
#[cfg(feature = "memoization")]
pub mod memo;

pub use graphene_core::*;

use dyn_any::{downcast_ref, DynAny, StaticType};
pub type DynNode<'n, T> = &'n (dyn Node<'n, Output = T> + 'n);
pub type DynAnyNode<'n> = &'n (dyn Node<'n, Output = &'n dyn DynAny<'n>> + 'n);

pub trait DynamicInput<'n> {
    fn set_kwarg_by_name(&mut self, name: &str, value: DynAnyNode<'n>);
    fn set_arg_by_index(&mut self, index: usize, value: DynAnyNode<'n>);
}

use quote::quote;
use syn::{Expr, ExprPath, Type};

/// Given a Node call tree, construct a function
/// that takes an input tuple and evaluates the call graph
/// on the gpu an fn node is constructed that takes a value
/// node as input
pub struct NodeGraph {
    /// Collection of nodes with their corresponding inputs.
    /// The first node always always has to be an Input Node.
    pub nodes: Vec<NodeKind>,
    pub output: Type,
    pub input: Type,
}
pub enum NodeKind {
    Value(Expr),
    Input,
    Node(ExprPath, Vec<usize>),
}

impl NodeGraph {
    pub fn serialize_function(&self) -> proc_macro2::TokenStream {
        let output_type = &self.output;
        let input_type = &self.input;

        fn nid(id: &usize) -> syn::Ident {
            let str = format!("n{id}");
            syn::Ident::new(str.as_str(), proc_macro2::Span::call_site())
        }
        let mut nodes = Vec::new();
        for (ref id, node) in self.nodes.iter().enumerate() {
            let id = nid(id).clone();
            let line = match node {
                NodeKind::Value(val) => {
                    quote! {let #id = graphene_core::value::ValueNode::new(#val);}
                }
                NodeKind::Node(node, ids) => {
                    let ids = ids.iter().map(nid).collect::<Vec<_>>();
                    quote! {let #id = #node::new((#(&#ids),*));}
                }
                NodeKind::Input => {
                    quote! { let n0 = graphene_core::value::ValueNode::new(input);}
                }
            };
            nodes.push(line)
        }
        let last_id = self.nodes.len() - 1;
        let last_id = nid(&last_id);
        let ret = quote! { #last_id.eval() };
        let function = quote! {
            fn node_graph(input: #input_type) -> #output_type {
                #(#nodes)*
                #ret
            }
        };
        function
    }
    pub fn serialize_gpu(&self, name: &str) -> proc_macro2::TokenStream {
        let function = self.serialize_function();
        let output_type = &self.output;
        let input_type = &self.input;

        quote! {
            #[cfg(target_arch = "spirv")]
            pub mod gpu {
                //#![deny(warnings)]
                #[repr(C)]
                pub struct PushConsts {
                    n: u32,
                    node: u32,
                }
                use super::*;

                use spirv_std::glam::UVec3;

                #[allow(unused)]
                #[spirv(compute(threads(64)))]
                pub fn #name(
                    #[spirv(global_invocation_id)] global_id: UVec3,
                    #[spirv(storage_buffer, descriptor_set = 0, binding = 0)] a: &[#input_type],
                    #[spirv(storage_buffer, descriptor_set = 0, binding = 1)] y: &mut [#output_type],
                    #[spirv(push_constant)] push_consts: &PushConsts,
                ) {
                    #function
                    let gid = global_id.x as usize;
                    // Only process up to n, which is the length of the buffers.
                    if global_id.x < push_consts.n {
                        y[gid] = node_graph(a[gid]);
                    }
                }
            }
        }
    }
}
