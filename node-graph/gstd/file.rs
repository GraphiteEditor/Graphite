# [cfg (target_arch = "spirv")]pub mod gpu {
# [repr (C )]pub struct PushConsts {
    n : u32 , node : u32 , 
    }
    use super :: * ;
    use spirv_std :: glam :: UVec3 ;
    # [allow (unused )]# [spirv (compute (threads (64)))]pub fn "add"(# [spirv (global_invocation_id )]global_id : UVec3 , # [spirv (storage_buffer , descriptor_set = 0, binding = 0)]a : & [u32 ], # [spirv (storage_buffer , descriptor_set = 0, binding = 1)]y : & mut [u32 ], # [spirv (push_constant )]push_consts : & PushConsts , ){
    fn node_graph (input : u32 )-> u32 {
        let n0 = graphene_core :: value :: ValueNode :: new (input );
            let n1 = graphene_core :: value :: ValueNode :: new (1u32);
            let n2 = graphene_core :: ops :: AddNode :: new ((& n0 , & n1 ));
            n2 . eval ()
        }
        let gid = global_id . x as usize ;
        if global_id . x < push_consts . n {
        y [gid ]= node_graph (a [gid ]);
            
        }
        
    }
    
}
