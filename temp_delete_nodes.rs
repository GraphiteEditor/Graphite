// This is a fixed version of the delete_nodes function for reference

pub fn delete_nodes(&mut self, nodes_to_delete: Vec<NodeId>, delete_children: bool, network_path: &[NodeId]) {
    let Some(outward_wires) = self.outward_wires(network_path).cloned() else {
        log::error!("Could not get outward wires in delete_nodes");
        return;
    };

    let mut delete_nodes = HashSet::new();
    for node_id in &nodes_to_delete {
        delete_nodes.insert(*node_id);

        if !delete_children {
            continue;
        };

        for upstream_id in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::LayerChildrenUpstreamFlow) {
            // Skip the original node since we're already deleting it
            if upstream_id == *node_id {
                continue;
            }
            
            // This does a downstream traversal starting from the current node, and ending at either a node in the `delete_nodes` set or the output.
            // If the traversal find as child node of a node in the `delete_nodes` set, then it is a sole dependent. If the output node is eventually reached, then it is not a sole dependent.
            let mut stack = vec![OutputConnector::node(upstream_id, 0)];
            let mut can_delete = true;
            
            // Check if this upstream node is connected to any node that's not in the delete set
            while let Some(current_node) = stack.pop() {
                let current_node_id = current_node.node_id().expect("The current node in the delete stack cannot be the export");
                
                // Get all downstream connections from this node
                let Some(downstream_nodes) = outward_wires.get(&current_node) else { continue };
                
                for downstream_node in downstream_nodes {
                    if let InputConnector::Node { node_id: downstream_id, .. } = downstream_node {
                        // If the downstream node is not in the delete set, this upstream node is connected to something we're keeping
                        if !delete_nodes.contains(downstream_id) && !nodes_to_delete.contains(downstream_id) {
                            can_delete = false;
                            break;
                        }
                        
                        let downstream_node_output = OutputConnector::node(*downstream_id, 0);
                        if !delete_nodes.contains(downstream_id) {
                            stack.push(downstream_node_output);
                        }
                        // Continue traversing over the downstream sibling, if the current node is a sibling to a node that will be deleted and it is a layer
                        else {
                            for deleted_node_id in &nodes_to_delete {
                                let Some(downstream_node) = self.document_node(deleted_node_id, network_path) else { continue };
                                let Some(input) = downstream_node.inputs.first() else { continue };

                                if let NodeInput::Node { node_id, .. } = input {
                                    if *node_id == current_node_id {
                                        stack.push(OutputConnector::node(*deleted_node_id, 0));
                                    }
                                }
                            }
                        }
                    }
                    // If the traversal reaches the export, then the current node is not a sole dependent
                    else {
                        can_delete = false;
                    }
                }
                
                if !can_delete {
                    break;
                }
            }
            
            if can_delete {
                delete_nodes.insert(upstream_id);
            }
        }
    }

    for delete_node_id in &delete_nodes {
        let upstream_chain_nodes = self
            .upstream_flow_back_from_nodes(vec![*delete_node_id], network_path, FlowType::PrimaryFlow)
            .skip(1)
            .take_while(|upstream_node| self.is_chain(upstream_node, network_path))
            .collect::<Vec<_>>();

        if !self.remove_references_from_network(delete_node_id, network_path) {
            log::error!("could not remove references from network");
            continue;
        }

        for input_index in 0..self.number_of_displayed_inputs(delete_node_id, network_path) {
            self.disconnect_input(&InputConnector::node(*delete_node_id, input_index), network_path);
        }

        let Some(network) = self.network_mut(network_path) else {
            log::error!("Could not get nested network in delete_nodes");
            continue;
        };

        network.nodes.remove(delete_node_id);
        self.transaction_modified();

        let Some(network_metadata) = self.network_metadata_mut(network_path) else {
            log::error!("Could not get nested network_metadata in delete_nodes");
            continue;
        };
        network_metadata.persistent_metadata.node_metadata.remove(delete_node_id);
        for previous_chain_node in upstream_chain_nodes {
            self.set_chain_position(&previous_chain_node, network_path);
        }
    }
    self.unload_all_nodes_bounding_box(network_path);
    // Instead of unloaded all node click targets, just unload the nodes upstream from the deleted nodes. unload_upstream_node_click_targets will not work since the nodes have been deleted.
    self.unload_all_nodes_click_targets(network_path);
    let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
        log::error!("Could not get selected nodes in NodeGraphMessage::DeleteNodes");
        return;
    };
    selected_nodes.retain_selected_nodes(|node_id| !nodes_to_delete.contains(node_id));
} 
