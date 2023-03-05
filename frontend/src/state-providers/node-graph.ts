import { reactive, readonly } from "vue";

import { type Editor } from "@/wasm-communication/editor";
import {
	type FrontendNode,
	type FrontendNodeLink,
	type FrontendNodeType,
	UpdateNodeGraph,
	UpdateNodeTypes,
	UpdateNodeGraphBarLayout,
	UpdateZoomWithScroll,
	defaultWidgetLayout,
	patchWidgetLayout,
} from "@/wasm-communication/messages";

// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
export function createNodeGraphState(editor: Editor) {
	const state = reactive({
		nodes: [] as FrontendNode[],
		links: [] as FrontendNodeLink[],
		nodeTypes: [] as FrontendNodeType[],
		nodeGraphBarLayout: defaultWidgetLayout(),
		zoomWithScroll: false as boolean,
	});

	// Set up message subscriptions on creation
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraph, (updateNodeGraph) => {
		state.nodes = updateNodeGraph.nodes;
		state.links = updateNodeGraph.links;
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeTypes, (updateNodeTypes) => {
		state.nodeTypes = updateNodeTypes.nodeTypes;
	});
	editor.subscriptions.subscribeJsMessage(UpdateNodeGraphBarLayout, (updateNodeGraphBarLayout) => {
		patchWidgetLayout(state.nodeGraphBarLayout, updateNodeGraphBarLayout);
	});
	editor.subscriptions.subscribeJsMessage(UpdateZoomWithScroll, (updateZoomWithScroll) => {
		state.zoomWithScroll = updateZoomWithScroll.zoomWithScroll;
	});

	return {
		state: readonly(state) as typeof state,
	};
}
export type NodeGraphState = ReturnType<typeof createNodeGraphState>;
