import json
import copy

def gen_id():
	new_id = 42
	while True:
		yield new_id
		new_id += 1


def gen_y():
	y = 7
	while True:
		yield y
		y += 3

new_id = gen_id()
y_position = gen_y()
new_nodes = {}
shift_left = 32

def update_layer(data, indent, layer_node_id, next_id):
	y = next(y_position)
	output = None
	if "Folder" in data:
		new_layer_ids = list(map(lambda x, _: x, new_id, data["Folder"]["layers"]))
		for index, layer in enumerate(reversed(data["Folder"]["layers"])):
			next_index = None
			if index +1 < len(new_layer_ids):
				next_index = new_layer_ids[index+1]
			update_layer(layer["data"], indent + 5, new_layer_ids[index], next_index)
		output = new_layer_ids[0]
	if "Layer" in data:
		network = data["Layer"]["network"]

		nodes = set(filter(lambda old_id: network["nodes"][old_id]["name"] != "Output", set(network["nodes"])))
		
		new_ids = dict(zip(map(lambda id: int(id), nodes), new_id))

		output_node = network["nodes"][str(network["outputs"][0]["node_id"])]
		shift_left = output_node["metadata"]["position"][0]
		output = new_ids[int(output_node["inputs"][0]["Node"]["node_id"])]
		
		for old_id in nodes:
			node = network["nodes"][old_id]
			for node_input in node["inputs"]:
				if "Node" in node_input:
					node_input["Node"]["node_id"] = new_ids[node_input["Node"]["node_id"]]
			if node["name"] == "Transform":
				
				
				node["implementation"]={"Unresolved":{"name": "graphene_core::transform::TransformNode<_, _, _, _, _, _>"}}
				node["manual_composition"]={"Concrete":{"name":"graphene_core::transform::Footprint","size":72,"align":8}}
			
			if node["name"] == "Shape":
				if not any(map(lambda x: network["nodes"][x]["name"] == "Cull", nodes)):
					node["metadata"]["position"][1] = y
					node["metadata"]["position"][0] -= shift_left + 8 + indent
					shape = next(new_id)

					new_nodes[str(shape)] = copy.deepcopy(node)

					node["name"] = "Cull"
					node["inputs"] = [{"Node":{"node_id":shape,"output_index":0,"lambda":False}}]
					node["manual_composition"] = {"Concrete":{"name":"graphene_core::transform::Footprint","size":72,"align":8}}
					node["has_primary_output"] = True
					node["implementation"] = {"Unresolved":{"name":"graphene_core::transform::CullNode<_>"}}
					node["metadata"]["position"][0] += shift_left + 8 + indent

			node["metadata"]["position"][1] = y
			node["metadata"]["position"][0] -= shift_left + indent

			new_nodes[str(new_ids[int(old_id)])] = node

		
		
	assert(output == None or str(output) in new_nodes)

	node_to_input = lambda node_id: {"Node": {"node_id": node_id,"output_index": 0,"lambda": False}} if node_id else {"Value":{"tagged_value":{"GraphicGroup":[]},"exposed":True}}

	node = {
		"name": "Layer",
		"inputs": [
			node_to_input(output),
			{
				"Value": {
					"tagged_value": {
						"String": ""
					},
					"exposed": False
				}
			},
			{
				"Value": {
					"tagged_value": {
						"BlendMode": "Normal"
					},
					"exposed": False
				}
			},
			{
				"Value": {
					"tagged_value": {
						"F32": 100.0
					},
					"exposed": False
				}
			},
			{
				"Value": {
					"tagged_value": {
						"Bool": True
					},
					"exposed": False
				}
			},
			{
				"Value": {
					"tagged_value": {
						"Bool": False
					},
					"exposed": False
				}
			},
			{
				"Value": {
					"tagged_value": {
						"Bool": False
					},
					"exposed": False
				}
			},
			node_to_input(next_id)
		],
		"manual_composition": None,
		"has_primary_output": True,
		"implementation": {
			"Network": {
				"inputs": [
					0,
					2,
					2,
					2,
					2,
					2,
					2,
					2
				],
				"outputs": [
					{
						"node_id": 2,
						"node_output_index": 0
					}
				],
				"nodes": {
					"1": {
						"name": "Monitor",
						"inputs": [
							{
								"Node": {
									"node_id": 0,
									"output_index": 0,
									"lambda": False
								}
							}
						],
						"manual_composition": {
							"Concrete": {
								"name": "graphene_core::transform::Footprint",
								"size": 72,
								"align": 8
							}
						},
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_core::memo::MonitorNode<_, _, _>"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": True,
						"world_state_hash": 0,
						"path": None
					},
					"2": {
						"name": "ConstructLayer",
						"inputs": [
							{
								"Node": {
									"node_id": 1,
									"output_index": 0,
									"lambda": False
								}
							},
							{
								"Network": {
									"Concrete": {
										"name": "alloc::string::String",
										"size": 12,
										"align": 4
									}
								}
							},
							{
								"Network": {
									"Concrete": {
										"name": "graphene_core::raster::adjustments::BlendMode",
										"size": 4,
										"align": 4
									}
								}
							},
							{
								"Network": {
									"Concrete": {
										"name": "f32",
										"size": 4,
										"align": 4
									}
								}
							},
							{
								"Network": {
									"Concrete": {
										"name": "bool",
										"size": 1,
										"align": 1
									}
								}
							},
							{
								"Network": {
									"Concrete": {
										"name": "bool",
										"size": 1,
										"align": 1
									}
								}
							},
							{
								"Network": {
									"Concrete": {
										"name": "bool",
										"size": 1,
										"align": 1
									}
								}
							},
							{
								"Network": {
									"Fn": [
										{
											"Concrete": {
												"name": "graphene_core::transform::Footprint",
												"size": 72,
												"align": 8
											}
										},
										{
											"Concrete": {
												"name": "graphene_core::graphic_element::GraphicGroup",
												"size": 12,
												"align": 4
											}
										}
									]
								}
							}
						],
						"manual_composition": {
							"Concrete": {
								"name": "graphene_core::transform::Footprint",
								"size": 72,
								"align": 8
							}
						},
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_core::ConstructLayerNode<_, _, _, _, _, _, _, _>"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": False,
						"world_state_hash": 0,
						"path": None
					},
					"0": {
						"name": "To Graphic Element",
						"inputs": [
							{
								"Network": {
									"Generic": "T"
								}
							}
						],
						"manual_composition": None,
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_core::ToGraphicElementData"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": False,
						"world_state_hash": 0,
						"path": None
					}
				},
				"disabled": [],
				"previous_outputs": None
			}
		},
		"metadata": {
			"position": [
				-indent,
				y
			]
		},
		"skip_deduplication": False,
		"world_state_hash": 0,
		"path": None
	}
	new_nodes[str(layer_node_id)] = node

def migrate(name, new_name):
	global new_id, new_id ,y_position, new_nodes
	new_id = gen_id()
	y_position = gen_y()
	new_nodes = {}

	with open(name) as f:
		document = json.load(f)
	data = document["document_legacy"]["root"]["data"]
	update_layer(data, 0, next(new_id), None)

	new_nodes["0"] = {
		"name": "Output",
		"inputs": [
			{
				"Node": {
					"node_id": 42,
					"output_index": 0,
					"lambda": False
				}
			},
			{
				"Network": {
					"Concrete": {
						"name": "graphene_core::application_io::EditorApi<graphene_std::wasm_application_io::WasmApplicationIo>",
						"size": 176,
						"align": 8
					}
				}
			}
		],
		"manual_composition": None,
		"has_primary_output": True,
		"implementation": {
			"Network": {
				"inputs": [
					3,
					0
				],
				"outputs": [
					{
						"node_id": 3,
						"node_output_index": 0
					}
				],
				"nodes": {
					"1": {
						"name": "Create Canvas",
						"inputs": [
							{
								"Node": {
									"node_id": 0,
									"output_index": 0,
									"lambda": False
								}
							}
						],
						"manual_composition": None,
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_std::wasm_application_io::CreateSurfaceNode"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": True,
						"world_state_hash": 0,
						"path": None
					},
					"3": {
						"name": "RenderNode",
						"inputs": [
							{
								"Node": {
									"node_id": 0,
									"output_index": 0,
									"lambda": False
								}
							},
							{
								"Network": {
									"Fn": [
										{
											"Concrete": {
												"name": "graphene_core::transform::Footprint",
												"size": 72,
												"align": 8
											}
										},
										{
											"Generic": "T"
										}
									]
								}
							},
							{
								"Node": {
									"node_id": 2,
									"output_index": 0,
									"lambda": False
								}
							}
						],
						"manual_composition": None,
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_std::wasm_application_io::RenderNode<_, _, _>"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": False,
						"world_state_hash": 0,
						"path": None
					},
					"2": {
						"name": "Cache",
						"inputs": [
							{
								"Node": {
									"node_id": 1,
									"output_index": 0,
									"lambda": False
								}
							}
						],
						"manual_composition": {
							"Concrete": {
								"name": "()",
								"size": 0,
								"align": 1
							}
						},
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_core::memo::MemoNode<_, _>"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": False,
						"world_state_hash": 0,
						"path": None
					},
					"0": {
						"name": "EditorApi",
						"inputs": [
							{
								"Network": {
									"Concrete": {
										"name": "graphene_core::application_io::EditorApi<graphene_std::wasm_application_io::WasmApplicationIo>",
										"size": 176,
										"align": 8
									}
								}
							}
						],
						"manual_composition": None,
						"has_primary_output": True,
						"implementation": {
							"Unresolved": {
								"name": "graphene_core::ops::IdNode"
							}
						},
						"metadata": {
							"position": [
								0,
								0
							]
						},
						"skip_deduplication": False,
						"world_state_hash": 0,
						"path": None
					}
				},
				"disabled": [],
				"previous_outputs": None
			}
		},
		"metadata": {
			"position": [
				8,
				4
			]
		},
		"skip_deduplication": False,
		"world_state_hash": 0,
		"path": None
	}

	document = {
		"document_legacy": {
			"root": {
				"visible": True,
				"name": None,
				"data": {
					"Folder": {
						"next_assignment_id": 0,
						"layer_ids": [],
						"layers": []
					}
				},
				"transform": {
					"matrix2": [
						1.0,
						0.0,
						0.0,
						1.0
					],
					"translation": [
						0.0,
						0.0
					]
				},
				"preserve_aspect": True,
				"pivot": [
					0.5,
					0.5
				],
				"blend_mode": "Normal",
				"opacity": 1.0
			},
			"document_network": {
				"inputs": [],
				"outputs": [
					{
						"node_id": 0,
						"node_output_index": 0
					}
				],
				"nodes": new_nodes,
				"disabled": [],
				"previous_outputs": None
			},
			"commit_hash": "ef46080400bc6c4e069765dd2127306abbc9a94b"
		},
		"saved_document_identifier": 0,
		"auto_saved_document_identifier": 0,
		"name": "Untitled Document 10",
		"version": "0.0.18",
		"document_mode": "DesignMode",
		"view_mode": "Normal",
		"overlays_visible": True,
		"layer_metadata": [],
		"layer_range_selection_reference": None,
		"navigation_handler": {
			"pan": [
				82.0,
				84.0
			],
			"tilt": 0.0,
			"zoom": 1.0,
			"transform_operation": "None",
			"mouse_position": [
				389.0,
				507.0
			],
			"finish_operation_with_click": False
		},
		"properties_panel_message_handler": {
			"active_selection": None
		}
	}


	with open(new_name, "w+") as f:
		json.dump(document, f, indent="\t")

migrate("just-a-potted-cactus.graphite","migrated_just_a_potted_cactus.graphite")
migrate("valley-of-spires.graphite","migrated_valley_of_spires.graphite")
