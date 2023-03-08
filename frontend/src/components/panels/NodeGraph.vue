<template>
	<LayoutCol class="node-graph">
		<LayoutRow class="options-bar"><WidgetLayout :layout="nodeGraphBarLayout" /></LayoutRow>
		<LayoutRow
			class="graph"
			ref="graph"
			@wheel="(e: WheelEvent) => scroll(e)"
			@pointerdown="(e: PointerEvent) => pointerDown(e)"
			@pointermove="(e: PointerEvent) => pointerMove(e)"
			@pointerup="(e: PointerEvent) => pointerUp(e)"
			@dblclick="(e: MouseEvent) => doubleClick(e)"
			:style="{
				'--grid-spacing': `${gridSpacing}px`,
				'--grid-offset-x': `${transform.x * transform.scale}px`,
				'--grid-offset-y': `${transform.y * transform.scale}px`,
				'--dot-radius': `${dotRadius}px`,
			}"
		>
			<LayoutCol class="node-list" data-node-list v-if="nodeListLocation" :style="{ marginLeft: `${nodeListX}px`, marginTop: `${nodeListY}px` }">
				<TextInput placeholder="Search Nodes..." :value="searchTerm" @update:value="(val) => (searchTerm = val)" v-focus />
				<LayoutCol v-for="nodeCategory in nodeCategories" :key="nodeCategory[0]">
					<TextLabel>{{ nodeCategory[0] }}</TextLabel>
					<TextButton v-for="nodeType in nodeCategory[1]" v-bind:key="String(nodeType)" :label="nodeType.name" :action="() => createNode(nodeType.name)" />
				</LayoutCol>
				<TextLabel v-if="nodeCategories.length === 0">No search results :(</TextLabel>
			</LayoutCol>
			<div
				class="nodes"
				ref="nodesContainer"
				:style="{
					transform: `scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`,
					transformOrigin: `0 0`,
				}"
			>
				<div
					v-for="node in nodes"
					:key="String(node.id)"
					class="node"
					:class="{ selected: selected.includes(node.id), previewed: node.previewed, disabled: node.disabled }"
					:style="{
						'--offset-left': (node.position?.x || 0) + (selected.includes(node.id) ? draggingNodes?.roundX || 0 : 0),
						'--offset-top': (node.position?.y || 0) + (selected.includes(node.id) ? draggingNodes?.roundY || 0 : 0),
					}"
					:data-node="node.id"
				>
					<div class="primary">
						<div class="ports">
							<div
								v-if="node.primaryInput"
								class="input port"
								data-port="input"
								:data-datatype="node.primaryInput"
								:style="{ '--data-color': `var(--color-data-${node.primaryInput})`, '--data-color-dim': `var(--color-data-${node.primaryInput}-dim)` }"
							>
								<div></div>
							</div>
							<div
								v-if="node.outputs.length > 0"
								class="output port"
								data-port="output"
								:data-datatype="node.outputs[0].dataType"
								:style="{ '--data-color': `var(--color-data-${node.outputs[0].dataType})`, '--data-color-dim': `var(--color-data-${node.outputs[0].dataType}-dim)` }"
							>
								<div></div>
							</div>
						</div>
						<IconLabel :icon="nodeIcon(node.displayName)" />
						<TextLabel>{{ node.displayName }}</TextLabel>
					</div>
					<div v-if="[...node.exposedInputs, ...node.outputs.slice(1)].length > 0" class="parameters">
						<div v-for="(parameter, index) in [...node.exposedInputs, ...node.outputs.slice(1)]" :key="index" class="parameter">
							<div class="ports">
								<div
									v-if="index < node.exposedInputs.length"
									class="input port"
									data-port="input"
									:data-datatype="parameter.dataType"
									:style="{
										'--data-color': `var(--color-data-${parameter.dataType})`,
										'--data-color-dim': `var(--color-data-${parameter.dataType}-dim)`,
									}"
								>
									<div></div>
								</div>
								<div
									v-else
									class="output port"
									data-port="output"
									:data-datatype="parameter.dataType"
									:style="{ '--data-color': `var(--color-data-${parameter.dataType})`, '--data-color-dim': `var(--color-data-${parameter.dataType}-dim)` }"
								>
									<div></div>
								</div>
							</div>
							<TextLabel :class="index < node.exposedInputs.length ? 'name' : 'output'">{{ parameter.name }}</TextLabel>
						</div>
					</div>
				</div>
			</div>
			<div
				class="wires"
				:style="{
					transform: `scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`,
					transformOrigin: `0 0`,
				}"
			>
				<svg>
					<path
						v-for="([pathString, dataType], index) in linkPaths"
						:key="index"
						:d="pathString"
						:style="{ '--data-color': `var(--color-data-${dataType})`, '--data-color-dim': `var(--color-data-${dataType}-dim)` }"
					/>
				</svg>
			</div>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.node-graph {
	height: 100%;
	position: relative;

	.node-list {
		width: max-content;
		position: fixed;
		padding: 5px;
		z-index: 3;
		background-color: var(--color-3-darkgray);

		.text-button + .text-button {
			margin-left: 0;
			margin-top: 4px;
		}
	}

	.options-bar {
		height: 32px;
		margin: 0 4px;
		flex: 0 0 auto;
		align-items: center;

		.widget-layout {
			flex-direction: row;
			flex-grow: 1;
			justify-content: space-between;
		}
	}

	.graph {
		position: relative;
		background: var(--color-2-mildblack);
		width: calc(100% - 8px);
		margin-left: 4px;
		margin-bottom: 4px;
		border-radius: 2px;
		overflow: hidden;

		// We're displaying the dotted grid in a pseudo-element because `image-rendering` is an inherited property and we don't want it to apply to child elements
		&::before {
			content: "";
			position: absolute;
			width: 100%;
			height: 100%;
			background-size: var(--grid-spacing) var(--grid-spacing);
			background-position: calc(var(--grid-offset-x) - var(--dot-radius)) calc(var(--grid-offset-y) - var(--dot-radius));
			background-image: radial-gradient(circle at var(--dot-radius) var(--dot-radius), var(--color-3-darkgray) var(--dot-radius), transparent 0);
			image-rendering: pixelated;
			mix-blend-mode: screen;
		}
	}

	.nodes,
	.wires {
		position: absolute;
		width: 100%;
		height: 100%;

		&.wires {
			width: 100%;
			height: 100%;
			pointer-events: none;

			svg {
				width: 100%;
				height: 100%;
				overflow: visible;

				path {
					fill: none;
					// stroke: var(--color-data-raster-dim);
					stroke: var(--data-color-dim);
					stroke-width: 2px;
				}
			}
		}

		&.nodes {
			.node {
				position: absolute;
				display: flex;
				flex-direction: column;
				min-width: 120px;
				border-radius: 4px;
				background: var(--color-4-dimgray);
				left: calc((var(--offset-left) + 0.5) * 24px);
				top: calc((var(--offset-top) - 0.5) * 24px);

				&.selected {
					border: 1px solid var(--color-e-nearwhite);
					margin: -1px;
				}

				&.disabled {
					background: var(--color-3-darkgray);
					color: var(--color-a-softgray);

					.icon-label {
						fill: var(--color-a-softgray);
					}
				}

				&.previewed {
					outline: 3px solid var(--color-data-vector);
				}

				.primary {
					display: flex;
					align-items: center;
					position: relative;
					gap: 4px;
					width: 100%;
					height: 24px;
					background: var(--color-5-dullgray);
					border-radius: 4px;

					.icon-label {
						margin-left: 4px;
					}

					.text-label {
						margin-right: 4px;
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
						height: 24px;
						width: calc(100% - 24px * 2);
						margin-left: 24px;
						margin-right: 24px;

						.text-label {
							width: 100%;

							&.output {
								text-align: right;
							}
						}
					}

					// Squares to cover up the rounded corners of the primary area and make them have a straight edge
					&::before,
					&::after {
						content: "";
						position: absolute;
						background: var(--color-5-dullgray);
						width: 4px;
						height: 4px;
						top: -4px;
					}

					&::before {
						left: 0;
					}

					&::after {
						right: 0;
					}
				}

				.ports {
					position: absolute;
					width: 100%;
					height: 100%;

					.port {
						position: absolute;
						margin: auto 0;
						top: 0;
						bottom: 0;
						width: 12px;
						height: 12px;
						border-radius: 50%;
						background: var(--data-color-dim);
						// background: var(--color-data-raster-dim);

						div {
							background: var(--data-color);
							// background: var(--color-data-raster);
							width: 8px;
							height: 8px;
							border-radius: 50%;
							position: absolute;
							top: 0;
							bottom: 0;
							left: 0;
							right: 0;
							margin: auto;
						}

						&.input {
							left: calc(-12px - 6px);
						}

						&.output {
							right: calc(-12px - 6px);
						}
					}
				}
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, nextTick } from "vue";

import type { IconName } from "@/utility-functions/icons";

import { UpdateNodeGraphSelection, type FrontendNodeLink } from "@/wasm-communication/messages";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import TextButton from "@/components/widgets/buttons/TextButton.vue";
import TextInput from "@/components/widgets/inputs/TextInput.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";
import WidgetLayout from "@/components/widgets/WidgetLayout.vue";

const WHEEL_RATE = (1 / 600) * 3;
const GRID_COLLAPSE_SPACING = 10;
const GRID_SIZE = 24;

export default defineComponent({
	inject: ["nodeGraph", "editor"],
	data() {
		return {
			transform: { scale: 1, x: 0, y: 0 },
			panning: false,
			selected: [] as bigint[],
			draggingNodes: undefined as { startX: number; startY: number; roundX: number; roundY: number } | undefined,
			selectIfNotDragged: undefined as undefined | bigint,
			linkInProgressFromConnector: undefined as HTMLDivElement | undefined,
			linkInProgressToConnector: undefined as HTMLDivElement | DOMRect | undefined,
			disconnecting: undefined as { nodeId: bigint; inputIndex: number; linkIndex: number } | undefined,
			nodeLinkPaths: [] as [string, string][],
			searchTerm: "",
			nodeListLocation: undefined as { x: number; y: number } | undefined,
		};
	},
	computed: {
		gridSpacing(): number {
			const dense = this.transform.scale * GRID_SIZE;
			let sparse = dense;

			while (sparse > 0 && sparse < GRID_COLLAPSE_SPACING) {
				sparse *= 2;
			}

			return sparse;
		},
		dotRadius(): number {
			return 1 + Math.floor(this.transform.scale - 0.5 + 0.001) / 2;
		},
		nodes() {
			return this.nodeGraph.state.nodes;
		},
		nodeGraphBarLayout() {
			return this.nodeGraph.state.nodeGraphBarLayout;
		},
		nodeCategories() {
			const categories = new Map();
			this.nodeGraph.state.nodeTypes.forEach((node) => {
				if (this.searchTerm.length && !node.name.toLowerCase().includes(this.searchTerm.toLowerCase()) && !node.category.toLowerCase().includes(this.searchTerm.toLowerCase())) return;

				const category = categories.get(node.category);
				if (category) category.push(node);
				else categories.set(node.category, [node]);
			});

			const result = Array.from(categories);
			return result;
		},
		nodeListX() {
			return ((this.nodeListLocation?.x || 0) * GRID_SIZE + this.transform.x) * this.transform.scale;
		},
		nodeListY() {
			return ((this.nodeListLocation?.y || 0) * GRID_SIZE + this.transform.y) * this.transform.scale;
		},
		linkPathInProgress(): [string, string] | undefined {
			if (this.linkInProgressFromConnector && this.linkInProgressToConnector) {
				return this.createWirePath(this.linkInProgressFromConnector, this.linkInProgressToConnector, false, false);
			}
			return undefined;
		},
		linkPaths(): [string, string][] {
			const linkPathInProgress = this.linkPathInProgress ? [this.linkPathInProgress] : [];
			return [...linkPathInProgress, ...this.nodeLinkPaths];
		},
	},
	watch: {
		nodes: {
			immediate: true,
			async handler() {
				this.selected = this.selected.filter((id) => this.nodeGraph.state.nodes.find((node) => node.id === id));
				await this.refreshLinks();
			},
		},
	},
	methods: {
		resolveLink(link: FrontendNodeLink, containerBounds: HTMLDivElement): { nodePrimaryOutput: HTMLDivElement | undefined; nodePrimaryInput: HTMLDivElement | undefined } {
			const outputIndex = Number(link.linkStartOutputIndex);
			const inputIndex = Number(link.linkEndInputIndex);

			const nodeOutputConnectors = containerBounds.querySelectorAll(`[data-node="${String(link.linkStart)}"] [data-port="output"]`) || undefined;
			const nodeInputConnectors = containerBounds.querySelectorAll(`[data-node="${String(link.linkEnd)}"] [data-port="input"]`) || undefined;

			const nodePrimaryOutput = nodeOutputConnectors?.[outputIndex] as HTMLDivElement | undefined;
			const nodePrimaryInput = nodeInputConnectors?.[inputIndex] as HTMLDivElement | undefined;

			return { nodePrimaryOutput, nodePrimaryInput };
		},
		async refreshLinks(): Promise<void> {
			await nextTick();

			const containerBounds = this.$refs.nodesContainer as HTMLDivElement | undefined;
			if (!containerBounds) return;

			const links = this.nodeGraph.state.links;
			this.nodeLinkPaths = links.flatMap((link, index) => {
				const { nodePrimaryInput, nodePrimaryOutput } = this.resolveLink(link, containerBounds);
				if (!nodePrimaryInput || !nodePrimaryOutput) return [];
				if (this.disconnecting?.linkIndex === index) return [];

				return [this.createWirePath(nodePrimaryOutput, nodePrimaryInput.getBoundingClientRect(), false, false)];
			});
		},
		nodeIcon(nodeName: string): IconName {
			const iconMap: Record<string, IconName> = {
				Output: "NodeOutput",
				Imaginate: "NodeImaginate",
				"Hue Shift Image": "NodeColorCorrection",
				"Brighten Image": "NodeColorCorrection",
				"Grayscale Image": "NodeColorCorrection",
			};
			return iconMap[nodeName] || "NodeNodes";
		},
		buildWirePathLocations(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): { x: number; y: number }[] {
			const containerBounds = (this.$refs.nodesContainer as HTMLDivElement | undefined)?.getBoundingClientRect();
			if (!containerBounds) return [];

			const outX = verticalOut ? outputBounds.x + outputBounds.width / 2 : outputBounds.x + outputBounds.width - 1;
			const outY = verticalOut ? outputBounds.y + 1 : outputBounds.y + outputBounds.height / 2;
			const outConnectorX = (outX - containerBounds.x) / this.transform.scale;
			const outConnectorY = (outY - containerBounds.y) / this.transform.scale;

			const inX = verticalIn ? inputBounds.x + inputBounds.width / 2 : inputBounds.x + 1;
			const inY = verticalIn ? inputBounds.y + inputBounds.height - 1 : inputBounds.y + inputBounds.height / 2;
			const inConnectorX = (inX - containerBounds.x) / this.transform.scale;
			const inConnectorY = (inY - containerBounds.y) / this.transform.scale;
			const horizontalGap = Math.abs(outConnectorX - inConnectorX);
			const verticalGap = Math.abs(outConnectorY - inConnectorY);

			const curveLength = 200;
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
		},
		buildWirePathString(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): string {
			const locations = this.buildWirePathLocations(outputBounds, inputBounds, verticalOut, verticalIn);
			if (locations.length === 0) return "[error]";
			return `M${locations[0].x},${locations[0].y} C${locations[1].x},${locations[1].y} ${locations[2].x},${locations[2].y} ${locations[3].x},${locations[3].y}`;
		},
		createWirePath(outputPort: HTMLDivElement, inputPort: HTMLDivElement | DOMRect, verticalOut: boolean, verticalIn: boolean): [string, string] {
			const inputPortRect = inputPort instanceof HTMLDivElement ? inputPort.getBoundingClientRect() : inputPort;

			const pathString = this.buildWirePathString(outputPort.getBoundingClientRect(), inputPortRect, verticalOut, verticalIn);
			const dataType = outputPort.getAttribute("data-datatype") || "general";

			return [pathString, dataType];
		},
		scroll(e: WheelEvent) {
			const [scrollX, scrollY] = [e.deltaX, e.deltaY];

			// If zoom with scroll is enabled: horizontal pan with Ctrl, vertical pan with Shift
			const zoomWithScroll = this.nodeGraph.state.zoomWithScroll;
			const zoom = zoomWithScroll ? !e.ctrlKey && !e.shiftKey : e.ctrlKey;
			const horizontalPan = zoomWithScroll ? e.ctrlKey : !e.ctrlKey && e.shiftKey;

			// Prevent the web page from being zoomed
			if (e.ctrlKey) e.preventDefault();

			// Always pan horizontally in response to a horizontal scroll wheel movement
			this.transform.x -= scrollX / this.transform.scale;

			// Zoom
			if (zoom) {
				let zoomFactor = 1 + Math.abs(scrollY) * WHEEL_RATE;
				if (scrollY > 0) zoomFactor = 1 / zoomFactor;

				const graphDiv: HTMLDivElement | undefined = (this.$refs.graph as typeof LayoutCol | undefined)?.$el;
				if (!graphDiv) return;
				const { x, y, width, height } = graphDiv.getBoundingClientRect();

				this.transform.scale *= zoomFactor;

				const newViewportX = width / zoomFactor;
				const newViewportY = height / zoomFactor;

				const deltaSizeX = width - newViewportX;
				const deltaSizeY = height - newViewportY;

				const deltaX = deltaSizeX * ((e.x - x) / width);
				const deltaY = deltaSizeY * ((e.y - y) / height);

				this.transform.x -= (deltaX / this.transform.scale) * zoomFactor;
				this.transform.y -= (deltaY / this.transform.scale) * zoomFactor;

				return;
			}

			// Pan
			if (horizontalPan) {
				this.transform.x -= scrollY / this.transform.scale;
			} else {
				this.transform.y -= scrollY / this.transform.scale;
			}
		},
		keydown(e: KeyboardEvent): void {
			if (e.key.toLowerCase() === "escape") {
				this.nodeListLocation = undefined;
				document.removeEventListener("keydown", this.keydown);
			}
		},
		// TODO: Move the event listener from the graph to the window so dragging outside the graph area (or even the whole browser window) works
		pointerDown(e: PointerEvent) {
			const [lmb, rmb] = [e.button === 0, e.button === 2];

			const port = (e.target as HTMLDivElement).closest("[data-port]") as HTMLDivElement;
			const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
			const nodeId = node?.getAttribute("data-node") || undefined;
			const nodeList = (e.target as HTMLElement).closest("[data-node-list]") as HTMLElement | undefined;
			const containerBounds = this.$refs.nodesContainer as HTMLDivElement | undefined;
			if (!containerBounds) return;

			// Create the add node popup on right click, then exit
			if (rmb) {
				const graphDiv: HTMLDivElement | undefined = (this.$refs.graph as typeof LayoutCol | undefined)?.$el;
				const graph = graphDiv?.getBoundingClientRect() || new DOMRect();
				this.nodeListLocation = {
					x: Math.round(((e.clientX - graph.x) / this.transform.scale - this.transform.x) / GRID_SIZE),
					y: Math.round(((e.clientY - graph.y) / this.transform.scale - this.transform.y) / GRID_SIZE),
				};

				document.addEventListener("keydown", this.keydown);
				return;
			}

			// If the user is clicking on the add nodes list, exit here
			if (lmb && nodeList) return;

			// Since the user is clicking elsewhere in the graph, ensure the add nodes list is closed
			if (lmb) this.nodeListLocation = undefined;

			// Alt-click sets the clicked node as previewed
			if (lmb && e.altKey && nodeId) {
				this.editor.instance.togglePreview(BigInt(nodeId));
			}

			// Clicked on a port dot
			if (lmb && port && node) {
				const isOutput = Boolean(port.getAttribute("data-port") === "output");

				if (isOutput) this.linkInProgressFromConnector = port;
				else {
					const inputNodeInPorts = Array.from(node.querySelectorAll(`[data-port="input"]`));
					const inputNodeConnectionIndexSearch = inputNodeInPorts.indexOf(port);
					const inputIndex = inputNodeConnectionIndexSearch > -1 ? inputNodeConnectionIndexSearch : undefined;
					// Set the link to draw from the input that a previous link was on
					if (inputIndex !== undefined && nodeId) {
						const nodeIdInt = BigInt(nodeId);
						const inputIndexInt = BigInt(inputIndex);
						const links = this.nodeGraph.state.links;
						const linkIndex = links.findIndex((value) => value.linkEnd === nodeIdInt && value.linkEndInputIndex === inputIndexInt);
						const nodeOutputConnectors = containerBounds.querySelectorAll(`[data-node="${String(links[linkIndex].linkStart)}"] [data-port="output"]`) || undefined;
						this.linkInProgressFromConnector = nodeOutputConnectors?.[Number(links[linkIndex].linkEndInputIndex)] as HTMLDivElement | undefined;
						const nodeInputConnectors = containerBounds.querySelectorAll(`[data-node="${String(links[linkIndex].linkEnd)}"] [data-port="input"]`) || undefined;
						this.linkInProgressToConnector = nodeInputConnectors?.[Number(links[linkIndex].linkEndInputIndex)] as HTMLDivElement | undefined;
						this.disconnecting = { nodeId: nodeIdInt, inputIndex, linkIndex };
						this.refreshLinks();
					}
				}

				return;
			}

			// Clicked on a node
			if (lmb && nodeId) {
				let modifiedSelected = false;

				const id = BigInt(nodeId);
				if (e.shiftKey || e.ctrlKey) {
					modifiedSelected = true;

					if (this.selected.includes(id)) this.selected.splice(this.selected.lastIndexOf(id), 1);
					else this.selected.push(id);
				} else if (!this.selected.includes(id)) {
					modifiedSelected = true;

					this.selected = [id];
				} else {
					this.selectIfNotDragged = id;
				}

				if (this.selected.includes(id)) {
					this.draggingNodes = { startX: e.x, startY: e.y, roundX: 0, roundY: 0 };
				}

				if (modifiedSelected) this.editor.instance.selectNodes(new BigUint64Array(this.selected));

				return;
			}

			// Clicked on the graph background
			if (lmb && this.selected.length !== 0) {
				this.selected = [];
				this.editor.instance.selectNodes(new BigUint64Array([]));
			}

			// LMB clicked on the graph background or MMB clicked anywhere
			this.panning = true;
		},
		doubleClick(e: MouseEvent) {
			const node = (e.target as HTMLElement).closest("[data-node]") as HTMLElement | undefined;
			const nodeId = node?.getAttribute("data-node") || undefined;
			if (nodeId) {
				const id = BigInt(nodeId);
				this.editor.instance.doubleClickNode(id);
			}
		},
		pointerMove(e: PointerEvent) {
			if (this.panning) {
				this.transform.x += e.movementX / this.transform.scale;
				this.transform.y += e.movementY / this.transform.scale;
			} else if (this.linkInProgressFromConnector) {
				const target = e.target as Element | undefined;
				const dot = (target?.closest(`[data-port="input"]`) || undefined) as HTMLDivElement | undefined;
				if (dot) {
					this.linkInProgressToConnector = dot;
				} else {
					this.linkInProgressToConnector = new DOMRect(e.x, e.y);
				}
			} else if (this.draggingNodes) {
				const deltaX = Math.round((e.x - this.draggingNodes.startX) / this.transform.scale / GRID_SIZE);
				const deltaY = Math.round((e.y - this.draggingNodes.startY) / this.transform.scale / GRID_SIZE);
				if (this.draggingNodes.roundX !== deltaX || this.draggingNodes.roundY !== deltaY) {
					this.draggingNodes.roundX = deltaX;
					this.draggingNodes.roundY = deltaY;
					this.refreshLinks();
				}
			}
		},
		pointerUp(e: PointerEvent) {
			const containerBounds = this.$refs.nodesContainer as HTMLDivElement | undefined;
			if (!containerBounds) return;
			this.panning = false;

			if (this.disconnecting) {
				this.editor.instance.disconnectNodes(BigInt(this.disconnecting.nodeId), this.disconnecting.inputIndex);
			}
			this.disconnecting = undefined;

			if (this.linkInProgressToConnector instanceof HTMLDivElement && this.linkInProgressFromConnector) {
				const outputNode = this.linkInProgressFromConnector.closest("[data-node]");
				const inputNode = this.linkInProgressToConnector.closest("[data-node]");

				const outputConnectedNodeID = outputNode?.getAttribute("data-node") ?? undefined;
				const inputConnectedNodeID = inputNode?.getAttribute("data-node") ?? undefined;

				if (outputNode && inputNode && outputConnectedNodeID && inputConnectedNodeID) {
					const inputNodeInPorts = Array.from(inputNode.querySelectorAll(`[data-port="input"]`));
					const outputNodeInPorts = Array.from(outputNode.querySelectorAll(`[data-port="output"]`));

					const inputNodeConnectionIndexSearch = inputNodeInPorts.indexOf(this.linkInProgressToConnector);
					const outputNodeConnectionIndexSearch = outputNodeInPorts.indexOf(this.linkInProgressFromConnector);

					const inputNodeConnectionIndex = inputNodeConnectionIndexSearch > -1 ? inputNodeConnectionIndexSearch : undefined;
					const outputNodeConnectionIndex = outputNodeConnectionIndexSearch > -1 ? outputNodeConnectionIndexSearch : undefined;

					if (inputNodeConnectionIndex !== undefined && outputNodeConnectionIndex !== undefined) {
						this.editor.instance.connectNodesByLink(BigInt(outputConnectedNodeID), outputNodeConnectionIndex, BigInt(inputConnectedNodeID), inputNodeConnectionIndex);
					}
				}
			} else if (this.draggingNodes) {
				if (this.draggingNodes.startX === e.x || this.draggingNodes.startY === e.y) {
					if (this.selectIfNotDragged !== undefined && (this.selected.length !== 1 || this.selected[0] !== this.selectIfNotDragged)) {
						this.selected = [this.selectIfNotDragged];
						this.editor.instance.selectNodes(new BigUint64Array(this.selected));
					}
				}

				if (this.selected.length > 0 && (this.draggingNodes.roundX !== 0 || this.draggingNodes.roundY !== 0))
					this.editor.instance.moveSelectedNodes(this.draggingNodes.roundX, this.draggingNodes.roundY);

				// Check if this node should be inserted between two other nodes
				if (this.selected.length === 1) {
					const selectedNodeId = this.selected[0];
					const selectedNode = containerBounds.querySelector(`[data-node="${String(selectedNodeId)}"]`);

					// Check that neither the input or output of the selected node are already connected.
					const notConnected =
						this.nodeGraph.state.links.findIndex((link) => link.linkStart === selectedNodeId || (link.linkEnd === selectedNodeId && link.linkEndInputIndex === BigInt(0))) === -1;
					const input = selectedNode?.querySelector(`[data-port="input"]`);
					const output = selectedNode?.querySelector(`[data-port="output"]`);

					// TODO: Make sure inputs are correctly typed
					if (selectedNode && notConnected && input && output) {
						// Find the link that the node has been dragged on top of
						const link = this.nodeGraph.state.links.find((link): boolean => {
							const { nodePrimaryInput, nodePrimaryOutput } = this.resolveLink(link, containerBounds);
							if (!nodePrimaryInput || !nodePrimaryOutput) return false;

							const wireCurveLocations = this.buildWirePathLocations(nodePrimaryOutput.getBoundingClientRect(), nodePrimaryInput.getBoundingClientRect(), false, false);

							const selectedNodeBounds = selectedNode.getBoundingClientRect();
							const containerBoundsBounds = containerBounds.getBoundingClientRect();

							return this.editor.instance.rectangleIntersects(
								new Float64Array(wireCurveLocations.map((loc) => loc.x)),
								new Float64Array(wireCurveLocations.map((loc) => loc.y)),
								selectedNodeBounds.top - containerBoundsBounds.y,
								selectedNodeBounds.left - containerBoundsBounds.x,
								selectedNodeBounds.bottom - containerBoundsBounds.y,
								selectedNodeBounds.right - containerBoundsBounds.x
							);
						});
						// If the node has been dragged on top of the link then connect it into the middle.
						if (link) {
							this.editor.instance.connectNodesByLink(link.linkStart, 0, selectedNodeId, 0);
							this.editor.instance.connectNodesByLink(selectedNodeId, 0, link.linkEnd, Number(link.linkEndInputIndex));
							this.editor.instance.shiftNode(selectedNodeId);
						}
					}
				}

				this.draggingNodes = undefined;
				this.selectIfNotDragged = undefined;
			}

			this.linkInProgressFromConnector = undefined;
			this.linkInProgressToConnector = undefined;
		},
		createNode(nodeType: string): void {
			if (!this.nodeListLocation) return;

			this.editor.instance.createNode(nodeType, this.nodeListLocation.x, this.nodeListLocation.y);
			this.nodeListLocation = undefined;
		},
	},
	mounted() {
		const outputPort1 = document.querySelectorAll(`[data-port="output"]`)[4] as HTMLDivElement | undefined;
		const inputPort1 = document.querySelectorAll(`[data-port="input"]`)[1] as HTMLDivElement | undefined;
		if (outputPort1 && inputPort1) this.createWirePath(outputPort1, inputPort1.getBoundingClientRect(), true, true);

		const outputPort2 = document.querySelectorAll(`[data-port="output"]`)[6] as HTMLDivElement | undefined;
		const inputPort2 = document.querySelectorAll(`[data-port="input"]`)[3] as HTMLDivElement | undefined;
		if (outputPort2 && inputPort2) this.createWirePath(outputPort2, inputPort2.getBoundingClientRect(), true, false);

		this.editor.subscriptions.subscribeJsMessage(UpdateNodeGraphSelection, (updateNodeGraphSelection) => {
			this.selected = updateNodeGraphSelection.selected;
		});
	},
	components: {
		IconLabel,
		LayoutCol,
		LayoutRow,
		TextLabel,
		TextButton,
		TextInput,
		WidgetLayout,
	},
});
</script>
