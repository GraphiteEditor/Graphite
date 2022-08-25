<template>
	<LayoutCol class="node-graph">
		<LayoutRow class="options-bar"></LayoutRow>
		<LayoutRow
			class="graph"
			@wheel="(e: WheelEvent) => scroll(e)"
			ref="graph"
			@pointerdown="(e: PointerEvent) => pointerDown(e)"
			@pointermove="(e: PointerEvent) => pointerMove(e)"
			@pointerup="(e: PointerEvent) => pointerUp(e)"
			:style="`--grid-spacing: ${gridSpacing}px; --grid-offset-x: ${transform.x * transform.scale}px; --grid-offset-y: ${transform.y * transform.scale}px; --dot-radius: ${dotRadius}px`"
		>
			<div
				class="nodes"
				ref="nodesContainer"
				:style="{
					transform: `scale(${transform.scale}) translate(${transform.x}px, ${transform.y}px)`,
					transformOrigin: `0 0`,
				}"
			>
				<div class="node" style="--offset-left: 3; --offset-top: 2; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port" data-datatype="raster">
							<div></div>
						</div> -->
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeImage'" :iconStyle="'Node'" />
						<TextLabel>Image</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 9; --offset-top: 2; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<div class="input port" data-datatype="raster">
								<div></div>
							</div>
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeImage'" :iconStyle="'Node'" />
						<TextLabel>Mask</TextLabel>
					</div>
					<div class="arguments">
						<div class="argument">
							<div class="ports">
								<div class="input port" data-datatype="raster" style="--data-color: var(--color-data-raster); --data-color-dim: var(--color-data-vector-dim)">
									<div></div>
								</div>
								<!-- <div class="output port" data-datatype="raster">
								<div></div>
							</div> -->
							</div>
							<TextLabel>Stencil</TextLabel>
						</div>
					</div>
				</div>
				<div class="node" style="--offset-left: 15; --offset-top: 2; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port" data-datatype="raster">
							<div></div>
						</div> -->
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeTransform'" :iconStyle="'Node'" />
						<TextLabel>Transform</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 21; --offset-top: 2; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<div class="input port" data-datatype="raster">
								<div></div>
							</div>
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeMotionBlur'" :iconStyle="'Node'" />
						<TextLabel>Motion Blur</TextLabel>
					</div>
					<div class="arguments">
						<div class="argument">
							<div class="ports">
								<div class="input port" data-datatype="raster">
									<div></div>
								</div>
								<!-- <div class="output port" data-datatype="raster">
								<div></div>
							</div> -->
							</div>
							<TextLabel>Strength</TextLabel>
						</div>
					</div>
				</div>
				<div class="node" style="--offset-left: 2; --offset-top: 5; --data-color: var(--color-data-vector); --data-color-dim: var(--color-data-vector-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port" data-datatype="vector">
							<div></div>
						</div> -->
							<div class="output port" data-datatype="vector">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeShape'" :iconStyle="'Node'" />
						<TextLabel>Shape</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 6; --offset-top: 7; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port" data-datatype="raster">
							<div></div>
						</div> -->
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeBrushwork'" :iconStyle="'Node'" />
						<TextLabel>Brushwork</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 12; --offset-top: 7; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port" data-datatype="raster">
							<div></div>
						</div> -->
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeBlur'" :iconStyle="'Node'" />
						<TextLabel>Blur</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 12; --offset-top: 9; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port" data-datatype="raster">
							<div></div>
						</div> -->
							<div class="output port" data-datatype="raster">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeGradient'" :iconStyle="'Node'" />
						<TextLabel>Gradient</TextLabel>
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
				<svg ref="wiresContainer"></svg>
			</div>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.node-graph {
	height: 100%;

	.options-bar {
		height: 32px;
		margin: 0 4px;
		flex: 0 0 auto;
		align-items: center;
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
				top: calc((var(--offset-top) + 0.5) * 24px);

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

				.arguments {
					display: flex;
					width: 100%;
					position: relative;

					.argument {
						position: relative;
						display: flex;
						align-items: center;
						height: 24px;
						width: 100%;
						margin-left: 24px;
						margin-right: 24px;
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
import { defineComponent } from "vue";

import LayoutCol from "@/components/layout/LayoutCol.vue";
import LayoutRow from "@/components/layout/LayoutRow.vue";
import IconLabel from "@/components/widgets/labels/IconLabel.vue";
import TextLabel from "@/components/widgets/labels/TextLabel.vue";

const WHEEL_RATE = 1 / 600;
const GRID_COLLAPSE_SPACING = 10;
const GRID_SIZE = 24;

export default defineComponent({
	data() {
		return {
			transform: { scale: 1, x: 0, y: 0 },
			panning: false,
			drawing: undefined as { port: HTMLElement; output: boolean; path: SVGElement } | undefined,
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
	},
	methods: {
		buildWirePathString(outputBounds: DOMRect, inputBounds: DOMRect, verticalOut: boolean, verticalIn: boolean): string {
			const containerBounds = (this.$refs.nodesContainer as HTMLElement).getBoundingClientRect();

			const outX = verticalOut ? outputBounds.x + outputBounds.width / 2 : outputBounds.x + outputBounds.width - 1;
			const outY = verticalOut ? outputBounds.y + 1 : outputBounds.y + outputBounds.height / 2;
			const outConnectorX = (outX - containerBounds.x) / this.transform.scale;
			const outConnectorY = (outY - containerBounds.y) / this.transform.scale;

			const inX = verticalIn ? inputBounds.x + inputBounds.width / 2 : inputBounds.x + 1;
			const inY = verticalIn ? inputBounds.y + inputBounds.height - 1 : inputBounds.y + inputBounds.height / 2;
			const inConnectorX = (inX - containerBounds.x) / this.transform.scale;
			const inConnectorY = (inY - containerBounds.y) / this.transform.scale;
			// debugger;
			const horizontalGap = Math.abs(outConnectorX - inConnectorX);
			const verticalGap = Math.abs(outConnectorY - inConnectorY);

			const curveLength = 200;
			const curveFalloffRate = curveLength * Math.PI * 2;

			const horizontalCurveAmount = -(2 ** ((-10 * horizontalGap) / curveFalloffRate)) + 1;
			const verticalCurveAmount = -(2 ** ((-10 * verticalGap) / curveFalloffRate)) + 1;
			const horizontalCurve = horizontalCurveAmount * curveLength;
			const verticalCurve = verticalCurveAmount * curveLength;

			return `M${outConnectorX},${outConnectorY} C${verticalOut ? outConnectorX : outConnectorX + horizontalCurve},${verticalOut ? outConnectorY - verticalCurve : outConnectorY} ${
				verticalIn ? inConnectorX : inConnectorX - horizontalCurve
			},${verticalIn ? inConnectorY + verticalCurve : inConnectorY} ${inConnectorX},${inConnectorY}`;
		},
		createWirePath(outputPort: HTMLElement, inputPort: HTMLElement, verticalOut: boolean, verticalIn: boolean): SVGPathElement {
			const pathString = this.buildWirePathString(outputPort.getBoundingClientRect(), inputPort.getBoundingClientRect(), verticalOut, verticalIn);
			const dataType = outputPort.dataset.datatype;

			const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
			path.setAttribute("d", pathString);
			path.setAttribute("style", `--data-color:  var(--color-data-${dataType}); --data-color-dim: var(--color-data-${dataType}-dim)`);
			(this.$refs.wiresContainer as HTMLElement).appendChild(path);

			return path;
		},
		scroll(e: WheelEvent) {
			const scroll = e.deltaY;
			let zoomFactor = 1 + Math.abs(scroll) * WHEEL_RATE;
			if (scroll > 0) zoomFactor = 1 / zoomFactor;

			const { x, y, width, height } = ((this.$refs.graph as typeof LayoutCol).$el as HTMLElement).getBoundingClientRect();

			this.transform.scale *= zoomFactor;

			const newViewportX = width / zoomFactor;
			const newViewportY = height / zoomFactor;

			const deltaSizeX = width - newViewportX;
			const deltaSizeY = height - newViewportY;

			const deltaX = deltaSizeX * ((e.x - x) / width);
			const deltaY = deltaSizeY * ((e.y - y) / height);

			this.transform.x -= (deltaX / this.transform.scale) * zoomFactor;
			this.transform.y -= (deltaY / this.transform.scale) * zoomFactor;
		},
		pointerDown(e: PointerEvent) {
			const port = (e.target as HTMLElement).closest(".port") as HTMLElement;

			if (port) {
				const output = port.classList.contains("output");
				const path = this.createWirePath(port, port, false, false);
				this.drawing = { port, output, path };
			} else {
				this.panning = true;
			}
			((this.$refs.graph as typeof LayoutCol).$el as HTMLElement).setPointerCapture(e.pointerId);
		},
		pointerMove(e: PointerEvent) {
			if (this.panning) {
				this.transform.x += e.movementX / this.transform.scale;
				this.transform.y += e.movementY / this.transform.scale;
			} else if (this.drawing) {
				const mouse = new DOMRect(e.x, e.y);
				const port = this.drawing.port.getBoundingClientRect();
				const output = this.drawing.output ? port : mouse;
				const input = this.drawing.output ? mouse : port;

				const pathString = this.buildWirePathString(output, input, false, false);
				this.drawing.path.setAttribute("d", pathString);
			}
		},
		pointerUp(e: PointerEvent) {
			((this.$refs.graph as typeof LayoutCol).$el as HTMLElement).releasePointerCapture(e.pointerId);
			this.panning = false;
			this.drawing = undefined;
		},
	},
	mounted() {
		const outputPort1 = document.querySelectorAll(".output.port")[4] as HTMLElement;
		const inputPort1 = document.querySelectorAll(".input.port")[1] as HTMLElement;
		this.createWirePath(outputPort1, inputPort1, true, true);

		const outputPort2 = document.querySelectorAll(".output.port")[6] as HTMLElement;
		const inputPort2 = document.querySelectorAll(".input.port")[3] as HTMLElement;
		this.createWirePath(outputPort2, inputPort2, true, false);
	},
	components: {
		IconLabel,
		LayoutCol,
		LayoutRow,
		TextLabel,
	},
});
</script>
