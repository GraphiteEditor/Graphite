<script lang="ts">
	import { getContext, onMount, tick } from "svelte";
	import { fade } from "svelte/transition";

	import { FADE_TRANSITION } from "@graphite/consts";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import type { IconName } from "@graphite/utility-functions/icons";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import type { FrontendNodeWire, FrontendNodeType, FrontendNode, FrontendGraphInput, FrontendGraphOutput, FrontendGraphDataType, WirePath } from "@graphite/wasm-communication/messages";

	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import BreadcrumbTrailButtons from "@graphite/components/widgets/buttons/BreadcrumbTrailButtons.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import RadioInput from "@graphite/components/widgets/inputs/RadioInput.svelte";
	import TextInput from "@graphite/components/widgets/inputs/TextInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;
	const ADD_NODE_MENU_WIDTH = 180;
	const ADD_NODE_MENU_HEIGHT = 200;

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	let graph: HTMLDivElement | undefined;
	let nodesContainer: HTMLDivElement | undefined;
	let nodeSearchInput: TextInput | undefined;

	// TODO: Using this not-complete code, or another better approach, make it so the dragged in-progress connector correctly handles showing/hiding the SVG shape of the connector caps
	// let wireInProgressFromLayerTop: bigint | undefined = undefined;
	// let wireInProgressFromLayerBottom: bigint | undefined = undefined;

	let nodeWirePaths: WirePath[] = [];
	let searchTerm = "";

	let inputs: SVGSVGElement[][] = [];
	let outputs: SVGSVGElement[][] = [];
	let nodeElements: HTMLDivElement[] = [];

	$: watchNodes($nodeGraph.nodes);

	$: gridSpacing = calculateGridSpacing($nodeGraph.transform.scale);
	$: dotRadius = 1 + Math.floor($nodeGraph.transform.scale - 0.5 + 0.001) / 2;
	$: nodeCategories = buildNodeCategories($nodeGraph.nodeTypes, searchTerm);

	$: (() => {
		if ($nodeGraph.contextMenuInformation?.contextMenuData == "CreateNode") {
			setTimeout(() => nodeSearchInput?.focus(), 0);
		}
	})();

	$: wirePaths = createWirePaths($nodeGraph.wirePathInProgress, nodeWirePaths);

	function calculateGridSpacing(scale: number): number {
		const dense = scale * GRID_SIZE;
		let sparse = dense;

		while (sparse > 0 && sparse < GRID_COLLAPSE_SPACING) {
			sparse *= 2;
		}

		return sparse;
	}

	type NodeCategoryDetails = {
		nodes: FrontendNodeType[];
		open: boolean;
	};

	function buildNodeCategories(nodeTypes: FrontendNodeType[], searchTerm: string): [string, NodeCategoryDetails][] {
		const categories = new Map<string, NodeCategoryDetails>();

		nodeTypes.forEach((node) => {
			let nameIncludesSearchTerm = node.name.toLowerCase().includes(searchTerm.toLowerCase());
			// Quick and dirty hack to alias "Layer" to "Merge" in the search
			if (node.name === "Merge") {
				nameIncludesSearchTerm = nameIncludesSearchTerm || "Layer".toLowerCase().includes(searchTerm.toLowerCase());
			}

			if (searchTerm.length > 0 && !nameIncludesSearchTerm && !node.category.toLowerCase().includes(searchTerm.toLowerCase())) {
				return;
			}

			const category = categories.get(node.category);
			let open = nameIncludesSearchTerm;
			if (searchTerm.length === 0) {
				open = false;
			}

			if (category) {
				category.open = open;
				category.nodes.push(node);
			} else
				categories.set(node.category, {
					open: open,
					nodes: [node],
				});
		});

		return Array.from(categories);
	}

	function createWirePaths(wirePathInProgress: WirePath | undefined, nodeWirePaths: WirePath[]): WirePath[] {
		const maybeWirePathInProgress = wirePathInProgress ? [wirePathInProgress] : [];
		return [...maybeWirePathInProgress, ...nodeWirePaths];
	}

	async function watchNodes(nodes: FrontendNode[]) {
		nodes.forEach((_, index) => {
			if (!inputs[index]) inputs[index] = [];
			if (!outputs[index]) outputs[index] = [];
		});

		await refreshWires();
	}

	function resolveWire(wire: FrontendNodeWire): { nodeOutput: SVGSVGElement | undefined; nodeInput: SVGSVGElement | undefined } {
		const outputIndex = Number(wire.wireStartOutputIndex);
		const inputIndex = Number(wire.wireEndInputIndex);

		const nodeOutputConnectors = outputs[$nodeGraph.nodes.findIndex((n) => n.id === wire.wireStart)];
		const nodeInputConnectors = inputs[$nodeGraph.nodes.findIndex((n) => n.id === wire.wireEnd)] || undefined;

		const nodeOutput = nodeOutputConnectors?.[outputIndex] as SVGSVGElement | undefined;
		const nodeInput = nodeInputConnectors?.[inputIndex] as SVGSVGElement | undefined;
		return { nodeOutput, nodeInput };
	}

	async function refreshWires() {
		await tick();

		const wires = $nodeGraph.wires;
		nodeWirePaths = wires.flatMap((wire) => {
			const { nodeInput, nodeOutput } = resolveWire(wire);
			if (!nodeInput || !nodeOutput) return [];

			const wireStart = $nodeGraph.nodes.find((n) => n.id === wire.wireStart)?.isLayer || false;
			const wireEnd = ($nodeGraph.nodes.find((n) => n.id === wire.wireEnd)?.isLayer && Number(wire.wireEndInputIndex) == 0) || false;

			return [createWirePath(nodeOutput, nodeInput.getBoundingClientRect(), wireStart, wireEnd, wire.dashed)];
		});
	}

	onMount(refreshWires);

	function nodeIcon(nodeName: string): IconName {
		const iconMap: Record<string, IconName> = {
			Output: "NodeOutput",
		};
		return iconMap[nodeName] || "NodeNodes";
	}

	function buildWirePathLocations(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): { x: number; y: number }[] {
		if (!nodesContainer) return [];

		const VERTICAL_WIRE_OVERLAP_ON_SHAPED_CAP = 1;

		const containerBounds = nodesContainer.getBoundingClientRect();

		const outX = verticalOut ? outputBounds.x + outputBounds.width / 2 : outputBounds.x + outputBounds.width - 1;
		const outY = verticalOut ? outputBounds.y + VERTICAL_WIRE_OVERLAP_ON_SHAPED_CAP : outputBounds.y + outputBounds.height / 2;
		const outConnectorX = (outX - containerBounds.x) / $nodeGraph.transform.scale;
		const outConnectorY = (outY - containerBounds.y) / $nodeGraph.transform.scale;

		const inX = verticalIn ? inputBounds.x + inputBounds.width / 2 : inputBounds.x + 1;
		const inY = verticalIn ? inputBounds.y + inputBounds.height - VERTICAL_WIRE_OVERLAP_ON_SHAPED_CAP : inputBounds.y + inputBounds.height / 2;
		const inConnectorX = (inX - containerBounds.x) / $nodeGraph.transform.scale;
		const inConnectorY = (inY - containerBounds.y) / $nodeGraph.transform.scale;
		const horizontalGap = Math.abs(outConnectorX - inConnectorX);
		const verticalGap = Math.abs(outConnectorY - inConnectorY);

		// TODO: Finish this commented out code replacement for the code below it based on this diagram: <https://files.keavon.com/-/InsubstantialElegantQueenant/capture.png>
		// // Straight: stacking lines which are always straight, or a straight horizontal wire between two aligned nodes
		// if ((verticalOut && verticalIn) || (!verticalOut && !verticalIn && verticalGap === 0)) {
		// 	return [
		// 		{ x: outConnectorX, y: outConnectorY },
		// 		{ x: inConnectorX, y: inConnectorY },
		// 	];
		// }

		// // L-shape bend
		// if (verticalOut !== verticalIn) {
		// }

		const curveLength = 24;
		const curveFalloffRate = curveLength * Math.PI * 2;

		const horizontalCurveAmount = -(2 ** ((-10 * horizontalGap) / curveFalloffRate)) + 1;
		const verticalCurveAmount = -(2 ** ((-10 * verticalGap) / curveFalloffRate)) + 1;
		const horizontalCurve = horizontalCurveAmount * curveLength;
		const verticalCurve = verticalCurveAmount * curveLength;

		return [
			{ x: outConnectorX, y: outConnectorY },
			{ x: verticalOut ? outConnectorX : outConnectorX + horizontalCurve, y: verticalOut ? outConnectorY - verticalCurve : outConnectorY },
			{ x: verticalIn ? inConnectorX : inConnectorX - horizontalCurve, y: verticalIn ? inConnectorY + verticalCurve : inConnectorY },
			{ x: inConnectorX, y: inConnectorY },
		];
	}

	function buildWirePathString(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): string {
		const locations = buildWirePathLocations(outputBounds, inputBounds, verticalOut, verticalIn);
		if (locations.length === 0) return "[error]";
		const SMOOTHING = 0.5;
		const delta01 = { x: (locations[1].x - locations[0].x) * SMOOTHING, y: (locations[1].y - locations[0].y) * SMOOTHING };
		const delta23 = { x: (locations[3].x - locations[2].x) * SMOOTHING, y: (locations[3].y - locations[2].y) * SMOOTHING };
		return `
			M${locations[0].x},${locations[0].y}
			L${locations[1].x},${locations[1].y}
			C${locations[1].x + delta01.x},${locations[1].y + delta01.y}
			${locations[2].x - delta23.x},${locations[2].y - delta23.y}
			${locations[2].x},${locations[2].y}
			L${locations[3].x},${locations[3].y}
			`
			.split("\n")
			.map((line) => line.trim())
			.join(" ");
	}

	function createWirePath(outputPort: SVGSVGElement, inputPort: SVGSVGElement | DOMRect, verticalOut: boolean, verticalIn: boolean, dashed: boolean): WirePath {
		const inputPortRect = inputPort instanceof DOMRect ? inputPort : inputPort.getBoundingClientRect();
		const outputPortRect = outputPort.getBoundingClientRect();

		const pathString = buildWirePathString(outputPortRect, inputPortRect, verticalOut, verticalIn);
		const dataType = (outputPort.getAttribute("data-datatype") as FrontendGraphDataType) || "General";

		return { pathString, dataType, thick: verticalIn && verticalOut, dashed };
	}

	function toggleLayerDisplay(displayAsLayer: boolean, toggleId: bigint) {
		let node = $nodeGraph.nodes.find((node) => node.id === toggleId);
		if (node !== undefined) {
			editor.handle.setToNodeOrLayer(node.id, displayAsLayer);
		}
	}

	function canBeToggledBetweenNodeAndLayer(toggleDisplayAsLayerNodeId: bigint) {
		return $nodeGraph.nodes.find((node) => node.id === toggleDisplayAsLayerNodeId)?.canBeLayer || false;
	}

	function createNode(nodeType: string) {
		if ($nodeGraph.contextMenuInformation === undefined) return;

		editor.handle.createNode(nodeType, $nodeGraph.contextMenuInformation.contextMenuCoordinates.x, $nodeGraph.contextMenuInformation.contextMenuCoordinates.y);
	}

	function nodeBorderMask(nodeWidth: number, primaryInputExists: boolean, parameters: number, primaryOutputExists: boolean, exposedOutputs: number): string {
		const nodeHeight = Math.max(1 + parameters, 1 + exposedOutputs) * 24;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Primary input
		if (primaryInputExists) boxes.push({ x: -8, y: 4, width: 16, height: 16 });
		// Parameter inputs
		for (let i = 0; i < parameters; i++) boxes.push({ x: -8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		// Primary output
		if (primaryOutputExists) boxes.push({ x: nodeWidth - 8, y: 4, width: 16, height: 16 });
		// Exposed outputs
		for (let i = 0; i < exposedOutputs; i++) boxes.push({ x: nodeWidth - 8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		return borderMask(boxes, nodeWidth, nodeHeight);
	}

	function layerBorderMask(nodeWidthFromThumbnail: number, nodeChainAreaLeftExtension: number): string {
		const NODE_HEIGHT = 2 * 24;
		const THUMBNAIL_WIDTH = 72 + 8 * 2;
		const FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT = 2;

		const nodeWidth = nodeWidthFromThumbnail + nodeChainAreaLeftExtension;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Left input
		if (nodeChainAreaLeftExtension > 0) {
			boxes.push({ x: -8, y: 16, width: 16, height: 16 });
		}

		// Thumbnail
		boxes.push({ x: nodeChainAreaLeftExtension - 8, y: -FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT, width: THUMBNAIL_WIDTH, height: NODE_HEIGHT + FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT * 2 });

		// Right visibility button
		boxes.push({ x: nodeWidth - 12, y: (NODE_HEIGHT - 24) / 2, width: 24, height: 24 });

		return borderMask(boxes, nodeWidth, NODE_HEIGHT);
	}

	function borderMask(boxes: { x: number; y: number; width: number; height: number }[], nodeWidth: number, nodeHeight: number): string {
		const rectangles = boxes.map((box) => `M${box.x},${box.y} L${box.x + box.width},${box.y} L${box.x + box.width},${box.y + box.height} L${box.x},${box.y + box.height}z`);
		return `M-2,-2 L${nodeWidth + 2},-2 L${nodeWidth + 2},${nodeHeight + 2} L-2,${nodeHeight + 2}z ${rectangles.join(" ")}`;
	}

	function dataTypeTooltip(value: FrontendGraphInput | FrontendGraphOutput): string {
		return value.resolvedType ? `Resolved Data: ${value.resolvedType}` : `Unresolved Data: ${value.dataType}`;
	}

	function connectedToText(output: FrontendGraphOutput): string {
		if (output.connected.length === 0) {
			return "Connected to nothing";
		} else {
			return output.connected.map((nodeId, index) => `Connected to ${nodeId}, port index ${output.connectedIndex[index]}`).join("\n");
		}
	}
</script>

<div
	class="graph"
	bind:this={graph}
	style:--grid-spacing={`${gridSpacing}px`}
	style:--grid-offset-x={`${$nodeGraph.transform.x}px`}
	style:--grid-offset-y={`${$nodeGraph.transform.y}px`}
	style:--dot-radius={`${dotRadius}px`}
	data-node-graph
>
	<BreadcrumbTrailButtons labels={["Document"].concat($nodeGraph.subgraphPath)} action={(index) => editor.handle.exitNestedNetwork($nodeGraph.subgraphPath?.length - index)} />
	<!-- Right click menu for adding nodes -->
	{#if $nodeGraph.contextMenuInformation}
		<LayoutCol
			class="context-menu"
			data-context-menu
			styles={{
				left: `${$nodeGraph.contextMenuInformation.contextMenuCoordinates.x * $nodeGraph.transform.scale + $nodeGraph.transform.x}px`,
				top: `${$nodeGraph.contextMenuInformation.contextMenuCoordinates.y * $nodeGraph.transform.scale + $nodeGraph.transform.y}px`,
				...($nodeGraph.contextMenuInformation.contextMenuData === "CreateNode"
					? {
							transform: `translate(0%, 0%)`,
							width: `${ADD_NODE_MENU_WIDTH}px`,
							height: `${ADD_NODE_MENU_HEIGHT}px`,
						}
					: {}),
			}}
		>
			{#if $nodeGraph.contextMenuInformation.contextMenuData === "CreateNode"}
				<TextInput placeholder="Search Nodes..." value={searchTerm} on:value={({ detail }) => (searchTerm = detail)} bind:this={nodeSearchInput} />
				<div class="list-results" on:wheel|passive|stopPropagation>
					{#each nodeCategories as nodeCategory}
						<details open={nodeCategory[1].open}>
							<summary>
								<TextLabel>{nodeCategory[0]}</TextLabel>
							</summary>
							{#each nodeCategory[1].nodes as nodeType}
								<TextButton label={nodeType.name} action={() => createNode(nodeType.name)} />
							{/each}
						</details>
					{:else}
						<TextLabel>No search results</TextLabel>
					{/each}
				</div>
			{:else}
				{@const contextMenuData = $nodeGraph.contextMenuInformation.contextMenuData}
				<LayoutRow class="toggle-layer-or-node">
					<TextLabel>Display as</TextLabel>
					<RadioInput
						selectedIndex={contextMenuData.currentlyIsNode ? 0 : 1}
						entries={[
							{
								value: "node",
								label: "Node",
								action: () => {
									toggleLayerDisplay(false, contextMenuData.nodeId);
								},
							},
							{
								value: "layer",
								label: "Layer",
								action: () => {
									toggleLayerDisplay(true, contextMenuData.nodeId);
								},
							},
						]}
						disabled={!canBeToggledBetweenNodeAndLayer(contextMenuData.nodeId)}
					/>
				</LayoutRow>
			{/if}
		</LayoutCol>
	{/if}
	<!-- Node connection wires -->
	<div class="wires" style:transform-origin={`0 0`} style:transform={`translate(${$nodeGraph.transform.x}px, ${$nodeGraph.transform.y}px) scale(${$nodeGraph.transform.scale})`}>
		<svg>
			{#each wirePaths as { pathString, dataType, thick, dashed }}
				<path
					d={pathString}
					style:--data-line-width={`${thick ? 8 : 2}px`}
					style:--data-color={`var(--color-data-${dataType.toLowerCase()})`}
					style:--data-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
					style:--data-dasharray={`3,${dashed ? 2 : 0}`}
				/>
			{/each}
		</svg>
	</div>
	<!-- Layers and nodes -->
	<div
		class="layers-and-nodes"
		style:transform-origin={`0 0`}
		style:transform={`translate(${$nodeGraph.transform.x}px, ${$nodeGraph.transform.y}px) scale(${$nodeGraph.transform.scale})`}
		bind:this={nodesContainer}
	>
		<!-- Layers -->
		{#each $nodeGraph.nodes.flatMap((node, nodeIndex) => (node.isLayer ? [{ node, nodeIndex }] : [])) as { node, nodeIndex } (nodeIndex)}
			{@const clipPathId = String(Math.random()).substring(2)}
			{@const stackDataInput = node.exposedInputs[0]}
			{@const layerAreaWidth = $nodeGraph.layerWidths.get(node.id) || 8}
			<div
				class="layer"
				class:selected={$nodeGraph.selected.includes(node.id)}
				class:previewed={node.previewed}
				class:disabled={!node.visible}
				style:--offset-left={(node.position?.x || 0) - 1}
				style:--offset-top={node.position?.y || 0}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()}-dim)`}
				style:--layer-area-width={layerAreaWidth}
				style:--node-chain-area-left-extension={node.exposedInputs.length === 0 ? 0 : 1.5}
				data-node={node.id}
				bind:this={nodeElements[nodeIndex]}
			>
				{#if node.errors}
					<span class="node-error faded" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
					<span class="node-error hover" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
				{/if}
				<div class="thumbnail">
					{#if $nodeGraph.thumbnails.has(node.id)}
						{@html $nodeGraph.thumbnails.get(node.id)}
					{/if}
					<!-- Layer stacking top output -->
					{#if node.primaryOutput}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 12"
							class="port top"
							data-port="output"
							data-datatype={node.primaryOutput.dataType}
							style:--data-color={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()}-dim)`}
							bind:this={outputs[nodeIndex][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryOutput)}\n${connectedToText(node.primaryOutput)}`}</title>
							{#if node.primaryOutput.connected.length > 0}
								<path d="M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z" fill="var(--data-color)" />
								{#if Number(node.primaryOutput?.connectedIndex) === 0 && $nodeGraph.nodes.find((n) => node.primaryOutput?.connected.includes(n.id))?.isLayer}
									<path d="M0,-3.5h8v8l-2.521,-1.681a2.666,2.666,0,0,0,-2.959,0l-2.52,1.681z" fill="var(--data-color-dim)" />
								{/if}
							{:else}
								<path d="M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					<!-- Layer stacking bottom input -->
					<svg
						xmlns="http://www.w3.org/2000/svg"
						viewBox="0 0 8 12"
						class="port bottom"
						data-port="input"
						data-datatype={node.primaryInput?.dataType}
						style:--data-color={`var(--color-data-${(node.primaryInput?.dataType || "General").toLowerCase()})`}
						style:--data-color-dim={`var(--color-data-${(node.primaryInput?.dataType || "General").toLowerCase()}-dim)`}
						bind:this={inputs[nodeIndex][0]}
					>
						{#if node.primaryInput}
							<title>{`${dataTypeTooltip(node.primaryInput)}\nConnected to ${node.primaryInput?.connected !== undefined ? node.primaryInput.connected : "nothing"}`}</title>
						{/if}
						{#if node.primaryInput?.connected !== undefined}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color)" />
							{#if $nodeGraph.nodes.find((n) => n.id === node.primaryInput?.connected)?.isLayer}
								<path d="M0,10.95l2.52,-1.69c0.89,-0.6,2.06,-0.6,2.96,0l2.52,1.69v5.05h-8v-5.05z" fill="var(--data-color-dim)" />
							{/if}
						{:else}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color-dim)" />
						{/if}
					</svg>
				</div>
				<!-- Layer input port (from left) -->
				{#if node.exposedInputs.length > 0}
					<div class="input ports">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port"
							data-port="input"
							data-datatype={stackDataInput.dataType}
							style:--data-color={`var(--color-data-${stackDataInput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${stackDataInput.dataType.toLowerCase()}-dim)`}
							bind:this={inputs[nodeIndex][1]}
						>
							<title>{`${dataTypeTooltip(stackDataInput)}\nConnected to ${stackDataInput.connected !== undefined ? stackDataInput.connected : "nothing"}`}</title>
							{#if stackDataInput.connected !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					</div>
				{/if}
				<div class="details">
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<span title={editor.handle.inDevelopmentMode() ? `Node ID: ${node.id}` : undefined}>
						{node.alias}
					</span>
				</div>
				<IconButton
					class={"visibility"}
					data-visibility-button
					size={24}
					icon={node.visible ? "EyeVisible" : "EyeHidden"}
					action={() => {
						/*Button is purely visual, clicking is handled in NodeGraphMessage::PointerDown*/
					}}
					tooltip={node.visible ? "Visible" : "Hidden"}
				/>

				<svg class="border-mask" width="0" height="0">
					<defs>
						<clipPath id={clipPathId}>
							<!-- Keep this equation in sync with the equivalent one in the CSS rule for `.layer { width: ... }` below -->
							<path clip-rule="evenodd" d={layerBorderMask(24 * layerAreaWidth - 12, node.exposedInputs.length === 0 ? 0 : 36)} />
						</clipPath>
					</defs>
				</svg>
			</div>
		{/each}
		<!-- Nodes -->
		{#each $nodeGraph.nodes.flatMap((node, nodeIndex) => (node.isLayer ? [] : [{ node, nodeIndex }])) as { node, nodeIndex } (nodeIndex)}
			{@const exposedInputsOutputs = [...node.exposedInputs, ...node.exposedOutputs]}
			{@const clipPathId = String(Math.random()).substring(2)}
			<div
				class="node"
				class:selected={$nodeGraph.selected.includes(node.id)}
				class:previewed={node.previewed}
				class:disabled={!node.visible}
				style:--offset-left={node.position?.x || 0}
				style:--offset-top={node.position?.y || 0}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()}-dim)`}
				data-node={node.id}
				bind:this={nodeElements[nodeIndex]}
			>
				{#if node.errors}
					<span class="node-error faded" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
					<span class="node-error hover" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
				{/if}
				<!-- Primary row -->
				<div class="primary" class:no-parameter-section={exposedInputsOutputs.length === 0}>
					<IconLabel icon={nodeIcon(node.name)} />
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<TextLabel tooltip={editor.handle.inDevelopmentMode() ? `Node ID: ${node.id}` : undefined}>{node.alias || node.name}</TextLabel>
				</div>
				<!-- Parameter rows -->
				{#if exposedInputsOutputs.length > 0}
					<div class="parameters">
						{#each exposedInputsOutputs as parameter, index}
							<div class={`parameter expanded ${index < node.exposedInputs.length ? "input" : "output"}`}>
								<TextLabel tooltip={parameter.name}>{parameter.name}</TextLabel>
							</div>
						{/each}
					</div>
				{/if}
				<!-- Input ports -->
				<div class="input ports">
					{#if node.primaryInput?.dataType}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port primary-port"
							data-port="input"
							data-datatype={node.primaryInput?.dataType}
							style:--data-color={`var(--color-data-${node.primaryInput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${node.primaryInput.dataType.toLowerCase()}-dim)`}
							bind:this={inputs[nodeIndex][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryInput)}\nConnected to ${node.primaryInput.connected !== undefined ? node.primaryInput.connected : "nothing"}`}</title>
							{#if node.primaryInput.connected !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedInputs as parameter, index}
						{#if index < node.exposedInputs.length}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="port"
								data-port="input"
								data-datatype={parameter.dataType}
								style:--data-color={`var(--color-data-${parameter.dataType.toLowerCase()})`}
								style:--data-color-dim={`var(--color-data-${parameter.dataType.toLowerCase()}-dim)`}
								bind:this={inputs[nodeIndex][index + (node.primaryInput ? 1 : 0)]}
							>
								<title>{`${dataTypeTooltip(parameter)}\nConnected to ${parameter.connected !== undefined ? parameter.connected : "nothing"}`}</title>
								{#if parameter.connected !== undefined}
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
								{:else}
									<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
								{/if}
							</svg>
						{/if}
					{/each}
				</div>
				<!-- Output ports -->
				<div class="output ports">
					{#if node.primaryOutput}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port primary-port"
							data-port="output"
							data-datatype={node.primaryOutput.dataType}
							style:--data-color={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${node.primaryOutput.dataType.toLowerCase()}-dim)`}
							bind:this={outputs[nodeIndex][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryOutput)}\n${connectedToText(node.primaryOutput)}`}</title>
							{#if node.primaryOutput.connected !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedOutputs as parameter, outputIndex}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port"
							data-port="output"
							data-datatype={parameter.dataType}
							style:--data-color={`var(--color-data-${parameter.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${parameter.dataType.toLowerCase()}-dim)`}
							bind:this={outputs[nodeIndex][outputIndex + (node.primaryOutput ? 1 : 0)]}
						>
							<title>{`${dataTypeTooltip(parameter)}\n${connectedToText(parameter)}`}</title>
							{#if parameter.connected !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/each}
				</div>
				<svg class="border-mask" width="0" height="0">
					<defs>
						<clipPath id={clipPathId}>
							<path
								clip-rule="evenodd"
								d={nodeBorderMask(120, node.primaryInput?.dataType !== undefined, node.exposedInputs.length, node.primaryOutput !== undefined, node.exposedOutputs.length)}
							/>
						</clipPath>
					</defs>
				</svg>
			</div>
		{/each}
	</div>
</div>

<!-- Box select widget -->
{#if $nodeGraph.box}
	<div
		class="box-selection"
		style:left={`${Math.min($nodeGraph.box.startX, $nodeGraph.box.endX)}px`}
		style:top={`${Math.min($nodeGraph.box.startY, $nodeGraph.box.endY)}px`}
		style:width={`${Math.abs($nodeGraph.box.startX - $nodeGraph.box.endX)}px`}
		style:height={`${Math.abs($nodeGraph.box.startY - $nodeGraph.box.endY)}px`}
	></div>
{/if}

<style lang="scss" global>
	.graph {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: row;
		flex-grow: 1;

		// We're displaying the dotted grid in a pseudo-element because `image-rendering` is an inherited property and we don't want it to apply to child elements
		&::before {
			content: "";
			position: absolute;
			width: 100%;
			height: 100%;
			background-size: var(--grid-spacing) var(--grid-spacing);
			background-position: calc(var(--grid-offset-x) - var(--dot-radius)) calc(var(--grid-offset-y) - var(--dot-radius));
			background-image: radial-gradient(circle at var(--dot-radius) var(--dot-radius), var(--color-3-darkgray) var(--dot-radius), transparent 0);
			background-repeat: repeat;
			image-rendering: pixelated;
			mix-blend-mode: screen;
		}

		> img {
			position: absolute;
			bottom: 0;
		}

		.breadcrumb-trail-buttons {
			margin-top: 8px;
			margin-left: 8px;
		}

		.context-menu {
			width: max-content;
			position: absolute;
			box-sizing: border-box;
			padding: 5px;
			z-index: 3;
			background-color: var(--color-3-darkgray);
			border-radius: 4px;

			.text-input {
				flex: 0 0 auto;
				margin-bottom: 4px;
			}

			.list-results {
				overflow-y: auto;
				flex: 1 1 auto;
				// Together with the `margin-right: 4px;` on `details` below, this keeps a gap between the listings and the scrollbar
				margin-right: -4px;

				details {
					cursor: pointer;
					display: flex;
					flex-direction: column;
					// Together with the `margin-right: -4px;` on `.list-results` above, this keeps a gap between the listings and the scrollbar
					margin-right: 4px;

					&[open] summary .text-label::before {
						transform: rotate(90deg);
					}

					summary {
						display: flex;
						align-items: center;
						gap: 2px;

						.text-label {
							padding-left: 16px;
							position: relative;
							width: 100%;

							&::before {
								content: "";
								position: absolute;
								margin: auto;
								top: 0;
								bottom: 0;
								left: 0;
								width: 8px;
								height: 8px;
								background: var(--icon-expand-collapse-arrow);
							}
						}
					}

					.text-button {
						width: 100%;
						margin: 4px 0;
					}
				}
			}

			.toggle-layer-or-node .text-label {
				line-height: 24px;
				margin-right: 8px;
			}
		}

		.wires {
			pointer-events: none;
			position: absolute;
			width: 100%;
			height: 100%;

			svg {
				width: 100%;
				height: 100%;
				overflow: visible;

				path {
					fill: none;
					stroke: var(--data-color-dim);
					stroke-width: var(--data-line-width);
					stroke-dasharray: var(--data-dasharray);
				}
			}
		}

		.layers-and-nodes {
			position: absolute;
			width: 100%;
			height: 100%;
		}

		.layer,
		.node {
			position: absolute;
			display: flex;
			left: calc(var(--offset-left) * 24px);
			top: calc(var(--offset-top) * 24px);
			// TODO: Reenable the `transition` property below after dealing with all edge cases where the wires need to be updated until the transition is complete
			// transition: top 0.1s cubic-bezier(0, 0, 0.2, 1), left 0.1s cubic-bezier(0, 0, 0.2, 1); // Update `DRAG_SMOOTHING_TIME` in the JS above.
			// TODO: Reenable the `backdrop-filter` property once a solution can be found for the black whole-page flickering problems it causes in Chrome.
			// TODO: Additionally, find a solution for this having no effect in Firefox due to a browser bug caused when the two
			// ancestor elements, `.graph` and `.panel`, each have the simultaneous pairing of `overflow: hidden` and `border-radius`.
			// See: https://stackoverflow.com/questions/75137879/bug-with-backdrop-filter-in-firefox
			// backdrop-filter: blur(4px);
			background: rgba(0, 0, 0, 0.33);

			.node-error {
				position: absolute;
				width: max-content;
				white-space: pre-wrap;
				max-width: 600px;
				line-height: 18px;
				color: var(--color-2-mildblack);
				background: var(--color-error-red);
				padding: 8px;
				border-radius: 4px;
				bottom: calc(100% + 12px);
				z-index: -1;
				transition: opacity 0.2s ease-in-out;
				opacity: 0.5;

				// Tail
				&::after {
					content: "";
					position: absolute;
					left: 6px;
					bottom: -8px;
					width: 0;
					height: 0;
					border-style: solid;
					border-width: 8px 6px 0 6px;
					border-color: var(--color-error-red) transparent transparent transparent;
				}

				&.hover {
					opacity: 0;
					z-index: 1;
					pointer-events: none;
				}

				&.faded:hover + .hover {
					opacity: 1;
				}

				&.faded:hover {
					z-index: 2;
					opacity: 1;
					-webkit-user-select: text;
					user-select: text;
					transition:
						opacity 0.2s ease-in-out,
						z-index 0s 0.2s;

					&::selection {
						background-color: var(--color-e-nearwhite);

						// Target only Safari
						@supports (background: -webkit-named-image(i)) {
							& {
								// Setting an alpha value opts out of Safari's "fancy" (but not visible on dark backgrounds) selection highlight rendering
								// https://stackoverflow.com/a/71753552/775283
								background-color: rgba(var(--color-e-nearwhite-rgb), calc(254 / 255));
							}
						}
					}
				}
			}

			&::after {
				content: "";
				position: absolute;
				box-sizing: border-box;
				top: 0;
				left: 0;
				width: 100%;
				height: 100%;
				pointer-events: none;
				clip-path: var(--clip-path-id);
			}

			.border-mask {
				position: absolute;
				top: 0;
			}

			&.disabled {
				background: var(--color-3-darkgray);
				color: var(--color-a-softgray);

				.icon-label {
					fill: var(--color-a-softgray);
				}
			}

			&.previewed::after {
				border: 1px dashed var(--data-color);
			}

			.ports {
				position: absolute;

				&.input {
					left: -3px;
				}

				&.output {
					right: -5px;
				}
			}

			.port {
				// Double the intended value because of margin collapsing, but for the first and last we divide it by two as intended
				margin: calc(24px - 8px) 0;
				width: 8px;
				height: 8px;
			}

			.text-label {
				overflow: hidden;
				text-overflow: ellipsis;
			}
		}

		.layer {
			border-radius: 8px;
			--extra-width-to-reach-grid-multiple: 8px;
			--node-chain-area-left-extension: 0;
			// Keep this equation in sync with the equivalent one in the Svelte template `<clipPath><path d="layerBorderMask(...)" /></clipPath>` above
			width: calc(24px * var(--layer-area-width) - 12px);
			padding-left: calc(var(--node-chain-area-left-extension) * 24px);
			margin-left: calc((1.5 - var(--node-chain-area-left-extension)) * 24px);

			&::after {
				border: 1px solid var(--color-5-dullgray);
				border-radius: 8px;
			}

			&.selected {
				// This is the result of blending `rgba(255, 255, 255, 0.1)` over `rgba(0, 0, 0, 0.33)`
				background: rgba(66, 66, 66, 0.4);
			}

			.thumbnail {
				background: var(--color-2-mildblack);
				border: 1px solid var(--data-color-dim);
				border-radius: 2px;
				position: relative;
				box-sizing: border-box;
				width: 72px;
				height: 48px;

				&::before {
					content: "";
					background: var(--color-transparent-checkered-background);
					background-size: var(--color-transparent-checkered-background-size);
					background-position: var(--color-transparent-checkered-background-position);
				}

				&::before,
				svg:not(.port) {
					pointer-events: none;
					position: absolute;
					margin: auto;
					top: 1px;
					left: 1px;
					width: calc(100% - 2px);
					height: calc(100% - 2px);
				}

				.port {
					position: absolute;
					margin: 0 auto;
					left: 0;
					right: 0;
					height: 12px;

					&.top {
						top: -13px;
					}

					&.bottom {
						bottom: -13px;
					}
				}
			}

			.details {
				margin: 0 8px;

				span {
					white-space: nowrap;
					line-height: 48px;
				}
			}

			.visibility {
				position: absolute;
				right: -12px;
			}

			.visibility,
			.input.ports,
			.input.ports .port {
				position: absolute;
				margin: auto 0;
				top: 0;
				bottom: 0;
			}

			.input.ports .port {
				left: 24px;
			}
		}

		.node {
			flex-direction: column;
			border-radius: 2px;
			width: 120px;
			top: calc((var(--offset-top) + 0.5) * 24px);

			&::after {
				border: 1px solid var(--data-color-dim);
				border-radius: 2px;
			}

			&.selected {
				.primary {
					background: rgba(255, 255, 255, 0.15);
				}

				.parameters {
					background: rgba(255, 255, 255, 0.1);
				}
			}

			.port {
				&:first-of-type {
					margin-top: calc((24px - 8px) / 2);

					&:not(.primary-port) {
						margin-top: calc((24px - 8px) / 2 + 24px);
					}
				}

				&:last-of-type {
					margin-bottom: calc((24px - 8px) / 2);
				}
			}

			.primary {
				display: flex;
				align-items: center;
				position: relative;
				width: 100%;
				height: 24px;
				border-radius: 2px 2px 0 0;
				background: rgba(255, 255, 255, 0.05);

				&.no-parameter-section {
					border-radius: 2px;
				}

				.icon-label {
					display: none; // Remove after we have unique icons for the nodes
					margin: 0 8px;
				}

				.text-label {
					// margin-right: 4px; // Restore after reenabling icon-label
					margin: 0 8px;
				}
			}

			.parameters {
				display: flex;
				flex-direction: column;
				width: 100%;
				position: relative;

				.parameter {
					position: relative;
					display: flex;
					align-items: center;
					margin: 0 8px;
					width: calc(100% - 8px - 8px);
					height: 24px;

					&:last-of-type {
						border-radius: 0 0 2px 2px;
					}

					.text-label {
						width: 100%;
					}

					&.output {
						flex-direction: row-reverse;
						text-align: right;

						svg {
							width: 30px;
							height: 20px;
						}
					}
				}

				&::before {
					left: 0;
				}

				&::after {
					right: 0;
				}
			}
		}
	}

	.box-selection {
		position: absolute;
		z-index: 2;
		background-color: rgba(77, 168, 221, 0.2);
		border: 1px solid rgba(77, 168, 221);
		pointer-events: none;
	}
</style>
