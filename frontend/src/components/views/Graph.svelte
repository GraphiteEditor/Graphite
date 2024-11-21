<script lang="ts">
	import { getContext, onMount, tick } from "svelte";
	import { fade } from "svelte/transition";

	import { FADE_TRANSITION } from "@graphite/consts";
	import type { NodeGraphState } from "@graphite/state-providers/node-graph";
	import type { IconName } from "@graphite/utility-functions/icons";
	import type { Editor } from "@graphite/wasm-communication/editor";
	import type { Node } from "@graphite/wasm-communication/messages";
	import type { FrontendNodeWire, FrontendNode, FrontendGraphInput, FrontendGraphOutput, FrontendGraphDataType, WirePath } from "@graphite/wasm-communication/messages";

	import NodeCatalog from "@graphite/components/floating-menus/NodeCatalog.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";
	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconButton from "@graphite/components/widgets/buttons/IconButton.svelte";
	import TextButton from "@graphite/components/widgets/buttons/TextButton.svelte";
	import RadioInput from "@graphite/components/widgets/inputs/RadioInput.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import Separator from "@graphite/components/widgets/labels/Separator.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";
	const GRID_COLLAPSE_SPACING = 10;
	const GRID_SIZE = 24;

	const editor = getContext<Editor>("editor");
	const nodeGraph = getContext<NodeGraphState>("nodeGraph");

	let graph: HTMLDivElement | undefined;
	let nodesContainer: HTMLDivElement | undefined;

	// TODO: Using this not-complete code, or another better approach, make it so the dragged in-progress connector correctly handles showing/hiding the SVG shape of the connector caps
	// let wireInProgressFromLayerTop: bigint | undefined = undefined;
	// let wireInProgressFromLayerBottom: bigint | undefined = undefined;

	let nodeWirePaths: WirePath[] = [];

	// TODO: Convert these arrays-of-arrays to a Map?
	let inputs: SVGSVGElement[][] = [];
	let outputs: SVGSVGElement[][] = [];
	let nodeElements: HTMLDivElement[] = [];

	$: watchNodes($nodeGraph.nodes);

	$: gridSpacing = calculateGridSpacing($nodeGraph.transform.scale);
	$: dotRadius = 1 + Math.floor($nodeGraph.transform.scale - 0.5 + 0.001) / 2;

	$: wirePaths = createWirePaths($nodeGraph.wirePathInProgress, nodeWirePaths);

	function calculateGridSpacing(scale: number): number {
		const dense = scale * GRID_SIZE;
		let sparse = dense;

		while (sparse > 0 && sparse < GRID_COLLAPSE_SPACING) {
			sparse *= 2;
		}

		return sparse;
	}

	function createWirePaths(wirePathInProgress: WirePath | undefined, nodeWirePaths: WirePath[]): WirePath[] {
		const maybeWirePathInProgress = wirePathInProgress ? [wirePathInProgress] : [];
		return [...maybeWirePathInProgress, ...nodeWirePaths];
	}

	async function watchNodes(nodes: Map<bigint, FrontendNode>) {
		Array.from(nodes.keys()).forEach((_, index) => {
			if (!inputs[index + 1]) inputs[index + 1] = [];
			if (!outputs[index + 1]) outputs[index + 1] = [];
		});
		if (!inputs[0]) inputs[0] = [];
		if (!outputs[0]) outputs[0] = [];

		await refreshWires();
	}

	function resolveWire(wire: FrontendNodeWire): { nodeOutput: SVGSVGElement | undefined; nodeInput: SVGSVGElement | undefined } {
		// TODO: Avoid the linear search
		const wireStartNodeIdIndex = Array.from($nodeGraph.nodes.keys()).findIndex((nodeId) => nodeId === (wire.wireStart as Node).nodeId);
		let nodeOutputConnectors = outputs[wireStartNodeIdIndex + 1];
		if (nodeOutputConnectors === undefined && (wire.wireStart as Node).nodeId === undefined) {
			nodeOutputConnectors = outputs[0];
		}
		const indexOutput = Number(wire.wireStart.index);
		const nodeOutput = nodeOutputConnectors?.[indexOutput] as SVGSVGElement | undefined;

		// TODO: Avoid the linear search
		const wireEndNodeIdIndex = Array.from($nodeGraph.nodes.keys()).findIndex((nodeId) => nodeId === (wire.wireEnd as Node).nodeId);
		let nodeInputConnectors = inputs[wireEndNodeIdIndex + 1] || undefined;
		if (nodeInputConnectors === undefined && (wire.wireEnd as Node).nodeId === undefined) {
			nodeInputConnectors = inputs[0];
		}
		const indexInput = Number(wire.wireEnd.index);
		const nodeInput = nodeInputConnectors?.[indexInput] as SVGSVGElement | undefined;

		return { nodeOutput, nodeInput };
	}

	function createWirePath(outputPort: SVGSVGElement, inputPort: SVGSVGElement, verticalOut: boolean, verticalIn: boolean, dashed: boolean): WirePath {
		const inputPortRect = inputPort.getBoundingClientRect();
		const outputPortRect = outputPort.getBoundingClientRect();

		const pathString = buildWirePathString(outputPortRect, inputPortRect, verticalOut, verticalIn);
		const dataType = (outputPort.getAttribute("data-datatype") as FrontendGraphDataType) || "General";
		const thick = verticalIn && verticalOut;

		return { pathString, dataType, thick, dashed };
	}

	async function refreshWires() {
		await tick();

		nodeWirePaths = $nodeGraph.wires.flatMap((wire) => {
			// TODO: This call contains linear searches, which combined with the loop we're in, causes O(n^2) complexity as the graph grows
			const { nodeOutput, nodeInput } = resolveWire(wire);
			if (!nodeOutput || !nodeInput) return [];

			const wireStartNode = wire.wireStart.nodeId !== undefined ? $nodeGraph.nodes.get(wire.wireStart.nodeId) : undefined;
			const wireStart = wireStartNode?.isLayer || false;

			const wireEndNode = wire.wireEnd.nodeId !== undefined ? $nodeGraph.nodes.get(wire.wireEnd.nodeId) : undefined;
			const wireEnd = (wireEndNode?.isLayer && Number(wire.wireEnd.index) === 0) || false;

			return [createWirePath(nodeOutput, nodeInput, wireStart, wireEnd, wire.dashed)];
		});
	}

	onMount(refreshWires);

	function nodeIcon(icon?: string): IconName {
		if (!icon) return "NodeNodes";
		const iconMap: Record<string, IconName> = {
			Output: "NodeOutput",
		};
		return iconMap[icon] || "NodeNodes";
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

	function toggleLayerDisplay(displayAsLayer: boolean, toggleId: bigint) {
		let node = $nodeGraph.nodes.get(toggleId);
		if (node) editor.handle.setToNodeOrLayer(node.id, displayAsLayer);
	}

	function canBeToggledBetweenNodeAndLayer(toggleDisplayAsLayerNodeId: bigint) {
		return $nodeGraph.nodes.get(toggleDisplayAsLayerNodeId)?.canBeLayer || false;
	}

	function createNode(nodeType: string) {
		if ($nodeGraph.contextMenuInformation === undefined) return;

		editor.handle.createNode(nodeType, $nodeGraph.contextMenuInformation.contextMenuCoordinates.x, $nodeGraph.contextMenuInformation.contextMenuCoordinates.y);
	}

	function nodeBorderMask(nodeWidth: number, primaryInputExists: boolean, exposedSecondaryInputs: number, primaryOutputExists: boolean, exposedSecondaryOutputs: number): string {
		const nodeHeight = Math.max(1 + exposedSecondaryInputs, 1 + exposedSecondaryOutputs) * 24;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Primary input
		if (primaryInputExists) boxes.push({ x: -8, y: 4, width: 16, height: 16 });
		// Secondary inputs
		for (let i = 0; i < exposedSecondaryInputs; i++) boxes.push({ x: -8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		// Primary output
		if (primaryOutputExists) boxes.push({ x: nodeWidth - 8, y: 4, width: 16, height: 16 });
		// Exposed outputs
		for (let i = 0; i < exposedSecondaryOutputs; i++) boxes.push({ x: nodeWidth - 8, y: 4 + (i + 1) * 24, width: 16, height: 16 });

		return borderMask(boxes, nodeWidth, nodeHeight);
	}

	function layerBorderMask(nodeWidthFromThumbnail: number, nodeChainAreaLeftExtension: number, hasLeftInputWire: boolean): string {
		const NODE_HEIGHT = 2 * 24;
		const THUMBNAIL_WIDTH = 72 + 8 * 2;
		const FUDGE_HEIGHT_BEYOND_LAYER_HEIGHT = 2;

		const nodeWidth = nodeWidthFromThumbnail + nodeChainAreaLeftExtension;

		const boxes: { x: number; y: number; width: number; height: number }[] = [];

		// Left input
		if (hasLeftInputWire && nodeChainAreaLeftExtension > 0) {
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

	function outputConnectedToText(output: FrontendGraphOutput): string {
		if (output.connectedTo.length === 0) {
			return "Connected to nothing";
		} else {
			return output.connectedTo
				.map((inputConnector) => {
					if ((inputConnector as Node).nodeId === undefined) {
						return `Connected to export index ${inputConnector.index}`;
					} else {
						return `Connected to ${(inputConnector as Node).nodeId}, port index ${inputConnector.index}`;
					}
				})
				.join("\n");
		}
	}

	function inputConnectedToText(input: FrontendGraphInput): string {
		if (input.connectedTo === undefined) {
			return "Connected to nothing";
		} else {
			if ((input.connectedTo as Node).nodeId === undefined) {
				return `Connected to import index ${input.connectedTo.index}`;
			} else {
				return `Connected to ${(input.connectedTo as Node).nodeId}, port index ${input.connectedTo.index}`;
			}
		}
	}

	function primaryOutputConnectedToLayer(node: FrontendNode): boolean {
		let firstConnectedNode = Array.from($nodeGraph.nodes.values()).find((n) =>
			node.primaryOutput?.connectedTo.some((connector) => {
				if ((connector as Node).nodeId === undefined) return false;
				if (connector.index !== 0n) return false;
				return n.id === (connector as Node).nodeId || false;
			}),
		);
		return firstConnectedNode?.isLayer || false;
	}

	function primaryInputConnectedToLayer(node: FrontendNode): boolean {
		const connectedNode = Array.from($nodeGraph.nodes.values()).find((n) => {
			if ((node.primaryInput?.connectedTo as Node) === undefined) return false;
			return n.id === (node.primaryInput?.connectedTo as Node).nodeId;
		});
		return connectedNode?.isLayer || false;
	}

	function zipWithUndefined(arr1: FrontendGraphInput[], arr2: FrontendGraphOutput[]) {
		const maxLength = Math.max(arr1.length, arr2.length);
		const result = [];
		for (let i = 0; i < maxLength; i++) {
			result.push([arr1[i], arr2[i]]);
		}
		return result;
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
	<!-- Right click menu for adding nodes -->
	{#if $nodeGraph.contextMenuInformation}
		<LayoutCol
			class="context-menu"
			data-context-menu
			styles={{
				left: `${$nodeGraph.contextMenuInformation.contextMenuCoordinates.x * $nodeGraph.transform.scale + $nodeGraph.transform.x}px`,
				top: `${$nodeGraph.contextMenuInformation.contextMenuCoordinates.y * $nodeGraph.transform.scale + $nodeGraph.transform.y}px`,
			}}
		>
			{#if $nodeGraph.contextMenuInformation.contextMenuData === "CreateNode"}
				<NodeCatalog on:selectNodeType={(e) => createNode(e.detail)} />
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
				<Separator type="Section" direction="Vertical" />
				<LayoutRow class="merge-selected-nodes">
					<TextButton label="Merge Selected Nodes" action={() => editor.handle.mergeSelectedNodes()} />
				</LayoutRow>
			{/if}
		</LayoutCol>
	{/if}

	<!-- Click target debug visualizations -->
	{#if $nodeGraph.clickTargets}
		<div class="click-targets" style:transform-origin={`0 0`} style:transform={`translate(${$nodeGraph.transform.x}px, ${$nodeGraph.transform.y}px) scale(${$nodeGraph.transform.scale})`}>
			<svg>
				{#each $nodeGraph.clickTargets.nodeClickTargets as pathString}
					<path class="node" d={pathString} />
				{/each}
				{#each $nodeGraph.clickTargets.layerClickTargets as pathString}
					<path class="layer" d={pathString} />
				{/each}
				{#each $nodeGraph.clickTargets.portClickTargets as pathString}
					<path class="port" d={pathString} />
				{/each}
				{#each $nodeGraph.clickTargets.iconClickTargets as pathString}
					<path class="visibility" d={pathString} />
				{/each}
				<path class="all-nodes-bounding-box" d={$nodeGraph.clickTargets.allNodesBoundingBox} />
				<path class="all-nodes-bounding-box" d={$nodeGraph.clickTargets.importExportsBoundingBox} />
			</svg>
		</div>
	{/if}

	<!-- Node connection wires -->
	<div class="wires" style:transform-origin={`0 0`} style:transform={`translate(${$nodeGraph.transform.x}px, ${$nodeGraph.transform.y}px) scale(${$nodeGraph.transform.scale})`}>
		<svg>
			{#each wirePaths as { pathString, dataType, thick, dashed }}
				{#if thick}
					<path
						d={pathString}
						style:--data-line-width={`${thick ? 8 : 2}px`}
						style:--data-color={`var(--color-data-${dataType.toLowerCase()})`}
						style:--data-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
						style:--data-dasharray={`3,${dashed ? 2 : 0}`}
					/>
				{/if}
			{/each}
		</svg>
	</div>

	<!-- Import and Export ports -->
	<div class="imports-and-exports" style:transform-origin={`0 0`} style:transform={`translate(${$nodeGraph.transform.x}px, ${$nodeGraph.transform.y}px) scale(${$nodeGraph.transform.scale})`}>
		{#each $nodeGraph.imports as { outputMetadata, position }, index}
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 8 8"
				class="port"
				data-port="output"
				data-datatype={outputMetadata.dataType}
				style:--data-color={`var(--color-data-${outputMetadata.dataType.toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${outputMetadata.dataType.toLowerCase()}-dim)`}
				style:--offset-left={position.x / 24}
				style:--offset-top={position.y / 24}
				bind:this={outputs[0][index]}
			>
				<title>{`${dataTypeTooltip(outputMetadata)}\n${outputConnectedToText(outputMetadata)}`}</title>
				{#if outputMetadata.connectedTo !== undefined}
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
				{:else}
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
				{/if}
			</svg>
			<p class="import-text" style:--offset-left={position.x / 24} style:--offset-top={position.y / 24}>{outputMetadata.name}</p>
		{/each}
		{#if $nodeGraph.addImport !== undefined}
			<div class="plus" style:--offset-left={$nodeGraph.addImport.x / 24} style:--offset-top={$nodeGraph.addImport.y / 24}>
				<IconButton
					class={"visibility"}
					data-visibility-button
					size={24}
					icon={"Add"}
					action={() => {
						/* Button is purely visual, clicking is handled in NodeGraphMessage::PointerDown */
					}}
				/>
			</div>
		{/if}
		{#each $nodeGraph.exports as { inputMetadata, position }, index}
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 8 8"
				class="port"
				data-port="input"
				data-datatype={inputMetadata.dataType}
				style:--data-color={`var(--color-data-${inputMetadata.dataType.toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${inputMetadata.dataType.toLowerCase()}-dim)`}
				style:--offset-left={position.x / 24}
				style:--offset-top={position.y / 24}
				bind:this={inputs[0][index]}
			>
				<title>{`${dataTypeTooltip(inputMetadata)}\n${inputConnectedToText(inputMetadata)}`}</title>
				{#if inputMetadata.connectedTo !== undefined}
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
				{:else}
					<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
				{/if}
			</svg>
			<p class="export-text" style:--offset-left={position.x / 24} style:--offset-top={position.y / 24}>{inputMetadata.name}</p>
		{/each}
		{#if $nodeGraph.addExport !== undefined}
			<div class="plus" style:--offset-left={$nodeGraph.addExport.x / 24} style:--offset-top={$nodeGraph.addExport.y / 24}>
				<IconButton
					class={"visibility"}
					data-visibility-button
					size={24}
					icon={"Add"}
					action={() => {
						/* Button is purely visual, clicking is handled in NodeGraphMessage::PointerDown */
					}}
				/>
			</div>
		{/if}
	</div>

	<!-- Layers and nodes -->
	<div
		class="layers-and-nodes"
		style:transform-origin={`0 0`}
		style:transform={`translate(${$nodeGraph.transform.x}px, ${$nodeGraph.transform.y}px) scale(${$nodeGraph.transform.scale})`}
		bind:this={nodesContainer}
	>
		<!-- Layers -->
		{#each Array.from($nodeGraph.nodes.values()).flatMap((node, nodeIndex) => (node.isLayer ? [{ node, nodeIndex }] : [])) as { node, nodeIndex } (nodeIndex)}
			{@const clipPathId = String(Math.random()).substring(2)}
			{@const stackDataInput = node.exposedInputs[0]}
			{@const layerAreaWidth = $nodeGraph.layerWidths.get(node.id) || 8}
			{@const layerChainWidth = $nodeGraph.chainWidths.get(node.id) || 0}
			{@const hasLeftInputWire = $nodeGraph.hasLeftInputWire.get(node.id) || false}
			{@const description = (node.reference && $nodeGraph.nodeDescriptions.get(node.reference)) || undefined}
			<div
				class="layer"
				class:selected={$nodeGraph.selected.includes(node.id)}
				class:in-selected-network={$nodeGraph.inSelectedNetwork}
				class:previewed={node.previewed}
				class:disabled={!node.visible}
				style:--offset-left={node.position?.x || 0}
				style:--offset-top={node.position?.y || 0}
				style:--clip-path-id={`url(#${clipPathId})`}
				style:--data-color={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()})`}
				style:--data-color-dim={`var(--color-data-${(node.primaryOutput?.dataType || "General").toLowerCase()}-dim)`}
				style:--layer-area-width={layerAreaWidth}
				style:--node-chain-area-left-extension={layerChainWidth !== 0 ? layerChainWidth + 0.5 : 0}
				title={`${node.displayName}\n\n${description || ""}`.trim() + (editor.handle.inDevelopmentMode() ? `\n\nNode ID: ${node.id}` : "")}
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
							bind:this={outputs[nodeIndex + 1][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryOutput)}\n${outputConnectedToText(node.primaryOutput)}`}</title>
							{#if node.primaryOutput.connectedTo.length > 0}
								<path d="M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z" fill="var(--data-color)" />
								{#if primaryOutputConnectedToLayer(node)}
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
						bind:this={inputs[nodeIndex + 1][0]}
					>
						{#if node.primaryInput}
							<title>{`${dataTypeTooltip(node.primaryInput)}\n${inputConnectedToText(node.primaryInput)}`}</title>
						{/if}
						{#if node.primaryInput?.connectedTo !== undefined}
							<path d="M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z" fill="var(--data-color)" />
							{#if primaryInputConnectedToLayer(node)}
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
							bind:this={inputs[nodeIndex + 1][1]}
						>
							<title>{`${dataTypeTooltip(stackDataInput)}\n${inputConnectedToText(stackDataInput)}`}</title>
							{#if stackDataInput.connectedTo !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					</div>
				{/if}
				<div class="details">
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<span>{node.displayName}</span>
				</div>
				<div class="solo-drag-grip" title="Drag only this layer without pushing others outside the stack"></div>
				<IconButton
					class={"visibility"}
					data-visibility-button
					size={24}
					icon={node.visible ? "EyeVisible" : "EyeHidden"}
					action={() => {
						/* Button is purely visual, clicking is handled in NodeGraphMessage::PointerDown */
					}}
					tooltip={node.visible ? "Visible" : "Hidden"}
				/>

				<svg class="border-mask" width="0" height="0">
					<defs>
						<clipPath id={clipPathId}>
							<!-- Keep this equation in sync with the equivalent one in the CSS rule for `.layer { width: ... }` below -->
							<path clip-rule="evenodd" d={layerBorderMask(24 * layerAreaWidth - 12, layerChainWidth ? (0.5 + layerChainWidth) * 24 : 0, hasLeftInputWire)} />
						</clipPath>
					</defs>
				</svg>
			</div>
		{/each}

		<!-- Node connection wires -->
		<div class="wires">
			<svg>
				{#each wirePaths as { pathString, dataType, thick, dashed }}\
					{#if !thick}
						<path
							d={pathString}
							style:--data-line-width={`${thick ? 8 : 2}px`}
							style:--data-color={`var(--color-data-${dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${dataType.toLowerCase()}-dim)`}
							style:--data-dasharray={dashed ? "4" : undefined}
						/>
					{/if}
				{/each}
			</svg>
		</div>

		<!-- Nodes -->
		{#each Array.from($nodeGraph.nodes.values()).flatMap((node, nodeIndex) => (node.isLayer ? [] : [{ node, nodeIndex }])) as { node, nodeIndex } (nodeIndex)}
			{@const exposedInputsOutputs = zipWithUndefined(node.exposedInputs, node.exposedOutputs)}
			{@const clipPathId = String(Math.random()).substring(2)}
			{@const description = (node.reference && $nodeGraph.nodeDescriptions.get(node.reference)) || undefined}
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
				title={`${node.displayName}\n\n${description || ""}`.trim() + (editor.handle.inDevelopmentMode() ? `\n\nNode ID: ${node.id}` : "")}
				data-node={node.id}
				bind:this={nodeElements[nodeIndex]}
			>
				{#if node.errors}
					<span class="node-error faded" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
					<span class="node-error hover" transition:fade={FADE_TRANSITION} data-node-error>{node.errors}</span>
				{/if}
				<!-- Primary row -->
				<div class="primary" class:in-selected-network={$nodeGraph.inSelectedNetwork} class:no-secondary-section={exposedInputsOutputs.length === 0}>
					<IconLabel icon={nodeIcon(node.reference)} />
					<!-- TODO: Allow the user to edit the name, just like in the Layers panel -->
					<TextLabel>{node.displayName}</TextLabel>
				</div>
				<!-- Secondary rows -->
				{#if exposedInputsOutputs.length > 0}
					<div class="secondary" class:in-selected-network={$nodeGraph.inSelectedNetwork}>
						{#each exposedInputsOutputs as [input, output]}
							<div class={`secondary-row expanded ${input !== undefined ? "input" : "output"}`}>
								<TextLabel tooltip={input !== undefined ? input.name : output.name}>
									{input !== undefined ? input.name : output.name}
								</TextLabel>
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
							bind:this={inputs[nodeIndex + 1][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryInput)}\n${inputConnectedToText(node.primaryInput)}`}</title>
							{#if node.primaryInput.connectedTo !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedInputs as secondary, index}
						{#if index < node.exposedInputs.length}
							<svg
								xmlns="http://www.w3.org/2000/svg"
								viewBox="0 0 8 8"
								class="port"
								data-port="input"
								data-datatype={secondary.dataType}
								style:--data-color={`var(--color-data-${secondary.dataType.toLowerCase()})`}
								style:--data-color-dim={`var(--color-data-${secondary.dataType.toLowerCase()}-dim)`}
								bind:this={inputs[nodeIndex + 1][index + (node.primaryInput ? 1 : 0)]}
							>
								<title>{`${dataTypeTooltip(secondary)}\n${inputConnectedToText(secondary)}`}</title>
								{#if secondary.connectedTo !== undefined}
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
							bind:this={outputs[nodeIndex + 1][0]}
						>
							<title>{`${dataTypeTooltip(node.primaryOutput)}\n${outputConnectedToText(node.primaryOutput)}`}</title>
							{#if node.primaryOutput.connectedTo !== undefined}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color)" />
							{:else}
								<path d="M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z" fill="var(--data-color-dim)" />
							{/if}
						</svg>
					{/if}
					{#each node.exposedOutputs as secondary, outputIndex}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							viewBox="0 0 8 8"
							class="port"
							data-port="output"
							data-datatype={secondary.dataType}
							style:--data-color={`var(--color-data-${secondary.dataType.toLowerCase()})`}
							style:--data-color-dim={`var(--color-data-${secondary.dataType.toLowerCase()}-dim)`}
							bind:this={outputs[nodeIndex + 1][outputIndex + (node.primaryOutput ? 1 : 0)]}
						>
							<title>{`${dataTypeTooltip(secondary)}\n${outputConnectedToText(secondary)}`}</title>
							{#if secondary.connectedTo !== undefined}
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

<!-- Box selection widget -->
<!-- TODO: Make its initial corner stay put (in graph space) when panning around -->
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

			.toggle-layer-or-node .text-label {
				line-height: 24px;
				margin-right: 8px;
			}

			.merge-selected-nodes {
				justify-content: center;
			}
		}

		.click-targets {
			position: absolute;
			pointer-events: none;
			width: 100%;
			height: 100%;
			z-index: 10;

			svg {
				overflow: visible;
				width: 100%;
				height: 100%;
				stroke-width: 1;
				fill: none;

				.layer {
					stroke: yellow;
				}

				.node {
					stroke: blue;
				}

				.port {
					stroke: green;
				}

				.visibility {
					stroke: red;
				}

				.all-nodes-bounding-box {
					stroke: purple;
				}
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

		.imports-and-exports {
			position: absolute;
			width: 100%;
			height: 100%;

			.port {
				position: absolute;
				width: 8px;
				height: 8px;
				margin-top: 4px;
				margin-left: 5px;
				top: calc(var(--offset-top) * 24px);
				left: calc(var(--offset-left) * 24px);
			}

			.plus {
				margin-top: -4px;
				margin-left: -4px;
				position: absolute;
				top: calc(var(--offset-top) * 24px);
				left: calc(var(--offset-left) * 24px);
			}

			.export-text {
				position: absolute;
				margin-top: 0;
				margin-left: 20px;
				top: calc(var(--offset-top) * 24px);
				left: calc(var(--offset-left) * 24px);
			}

			.import-text {
				position: absolute;
				text-align: right;
				top: calc(var(--offset-top) * 24px);
				left: calc(var(--offset-left) * 24px);
				margin-top: 0;
				margin-left: calc(-100px - 2px);
				width: 100px;
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
			background: rgba(var(--color-0-black-rgb), 0.33);

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
				background: rgba(var(--color-4-dimgray-rgb), 0.33);
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
			// Keep this equation in sync with the equivalent one in the Svelte template `<clipPath><path d="layerBorderMask(...)" /></clipPath>` above, as well as the `left` port offset CSS rule above in `.ports.input` above.
			width: calc((var(--layer-area-width) - 0.5) * 24px);
			padding-left: calc(var(--node-chain-area-left-extension) * 24px);
			margin-left: calc((0.5 - var(--node-chain-area-left-extension)) * 24px);

			&::after {
				border: 1px solid var(--color-5-dullgray);
				border-radius: 8px;
			}

			&.selected {
				background: rgba(var(--color-5-dullgray-rgb), 0.33);

				&.in-selected-network {
					background: rgba(var(--color-6-lowergray-rgb), 0.33);
				}
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
					background-image: var(--color-transparent-checkered-background);
					background-size: var(--color-transparent-checkered-background-size);
					background-position: var(--color-transparent-checkered-background-position);
					background-repeat: var(--color-transparent-checkered-background-repeat);
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

			.solo-drag-grip {
				width: 8px;
				height: 24px;
				background-position: 2px 8px;
				right: calc(-12px + 24px);
				border-radius: 2px;
			}

			.solo-drag-grip:hover,
			&.selected .solo-drag-grip {
				background-image: var(--icon-drag-grip);

				&:hover {
					background-image: var(--icon-drag-grip-hover);
				}
			}

			.visibility {
				position: absolute;
				right: -12px;
			}

			.input.ports {
				left: calc(-3px + var(--node-chain-area-left-extension) * 24px - 36px);
			}

			.solo-drag-grip,
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
					background: rgba(var(--color-f-white-rgb), 0.15);

					&.in-selected-network {
						background: rgba(var(--color-f-white-rgb), 0.2);
					}
				}

				.secondary {
					background: rgba(var(--color-f-white-rgb), 0.1);

					&.in-selected-network {
						background: rgba(var(--color-f-white-rgb), 0.15);
					}
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
				background: rgba(var(--color-f-white-rgb), 0.05);

				&.no-secondary-section {
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

			.secondary {
				display: flex;
				flex-direction: column;
				width: 100%;
				position: relative;

				.secondary-row {
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
		pointer-events: none;
		background: rgba(var(--color-overlay-blue-rgb), 0.05);
		border: 1px solid var(--color-overlay-blue);
		z-index: 2;
	}
</style>
