<template>
	<LayoutCol class="node-graph">
		<LayoutRow class="options-bar"></LayoutRow>
		<LayoutRow class="graph" @wheel="(e) => scroll(e)" ref="graph" @pointerdown="(e) => pointerDown(e)" @pointermove="(e) => pointerMove(e)" @pointerup="(e) => pointerUp(e)">
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
							<!-- <div class="input port">
							<div></div>
						</div> -->
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeImage'" :style="'node'" />
						<TextLabel>Image</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 9; --offset-top: 2; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<div class="input port">
								<div></div>
							</div>
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeImage'" :style="'node'" />
						<TextLabel>Mask</TextLabel>
					</div>
					<div class="arguments">
						<div class="argument">
							<div class="ports">
								<div class="input port" style="--data-color: var(--color-data-raster); --data-color-dim: var(--color-data-vector-dim)">
									<div></div>
								</div>
								<!-- <div class="output port">
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
							<!-- <div class="input port">
							<div></div>
						</div> -->
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeTransform'" :style="'node'" />
						<TextLabel>Transform</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 21; --offset-top: 2; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<div class="input port">
								<div></div>
							</div>
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeMotionBlur'" :style="'node'" />
						<TextLabel>Motion Blur</TextLabel>
					</div>
					<div class="arguments">
						<div class="argument">
							<div class="ports">
								<div class="input port">
									<div></div>
								</div>
								<!-- <div class="output port">
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
							<!-- <div class="input port">
							<div></div>
						</div> -->
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeShape'" :style="'node'" />
						<TextLabel>Shape</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 6; --offset-top: 7; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port">
							<div></div>
						</div> -->
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeBrushwork'" :style="'node'" />
						<TextLabel>Brushwork</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 12; --offset-top: 7; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port">
							<div></div>
						</div> -->
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeBlur'" :style="'node'" />
						<TextLabel>Blur</TextLabel>
					</div>
				</div>
				<div class="node" style="--offset-left: 12; --offset-top: 9; --data-color: var(--color-data-raster); --data-color-dim: var(--color-data-raster-dim)">
					<div class="primary">
						<div class="ports">
							<!-- <div class="input port">
							<div></div>
						</div> -->
							<div class="output port">
								<div></div>
							</div>
						</div>
						<IconLabel :icon="'NodeGradient'" :style="'node'" />
						<TextLabel>Gradient</TextLabel>
					</div>
				</div>
			</div>
			<div class="wires">
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
		background-image: radial-gradient(circle at 1px 1px, var(--color-3-darkgray) 1px, transparent 0);
		background-size: 24px 24px;
		background-position: -1px -1px;
		width: calc(100% - 8px);
		margin-left: 4px;
		margin-bottom: 4px;
		border-radius: 2px;
		overflow: hidden;
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
				border-radius: 2px;
				background: var(--color-4-dimgray);
				left: calc(var(--offset-left) * 24px);
				top: calc(var(--offset-top) * 24px);

				.primary {
					display: flex;
					align-items: center;
					position: relative;
					gap: 4px;
					width: 100%;
					height: 24px;
					background: var(--color-6-lowergray);
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
						background: var(--color-6-lowergray);
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

export default defineComponent({
	inject: ["editor"],
	data() {
		return { transform: { scale: 2, x: 0, y: 0 }, panning: false };
	},
	methods: {
		buildWirePathString(outputPort: HTMLElement, inputPort: HTMLElement): string {
			const containerBounds = (this.$refs.nodesContainer as HTMLElement).getBoundingClientRect();
			const outputBounds = outputPort.getBoundingClientRect();
			const inputBounds = inputPort.getBoundingClientRect();

			const outConnectorX = outputBounds.x + (outputBounds.width - 1) - containerBounds.x;
			const outConnectorY = outputBounds.y + outputBounds.height / 2 - containerBounds.y;
			const inConnectorX = inputBounds.x + 1 - containerBounds.x;
			const inConnectorY = inputBounds.y + inputBounds.height / 2 - containerBounds.y;
			// debugger;

			const horizontalGap = Math.abs(outConnectorX - inConnectorX);
			const curveLength = 200;
			const curveFalloffRate = curveLength * Math.PI * 2;
			const curveAmount = -(2 ** ((-10 * horizontalGap) / curveFalloffRate)) + 1;
			const curve = curveAmount * curveLength;

			return `M${outConnectorX},${outConnectorY} C${outConnectorX + curve},${outConnectorY} ${inConnectorX - curve},${inConnectorY} ${inConnectorX},${inConnectorY}`;
		},
		createWirePath(outputPort: HTMLElement, inputPort: HTMLElement) {
			const pathString = this.buildWirePathString(outputPort, inputPort);
			const dataType = "vector";

			const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
			path.setAttribute("d", pathString);
			path.setAttribute("style", `--data-color: var(--color-data-${dataType}); --data-color-dim: var(--color-data-${dataType}-dim)`);
			(this.$refs.wiresContainer as HTMLElement).appendChild(path);
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
			((this.$refs.graph as typeof LayoutCol).$el as HTMLElement).setPointerCapture(e.pointerId);
			this.panning = true;
		},
		pointerMove(e: PointerEvent) {
			if (this.panning) {
				this.transform.x += e.movementX / this.transform.scale;
				this.transform.y += e.movementY / this.transform.scale;
			}
		},
		pointerUp(e: PointerEvent) {
			((this.$refs.graph as typeof LayoutCol).$el as HTMLElement).releasePointerCapture(e.pointerId);
			this.panning = false;
		},
	},
	mounted() {
		const outputPort = document.querySelectorAll(".output.port")[4] as HTMLElement;
		const inputPort = document.querySelectorAll(".input.port")[1] as HTMLElement;
		this.createWirePath(outputPort, inputPort);
	},
	components: {
		LayoutRow,
		LayoutCol,
		IconLabel,
		TextLabel,
	},
});
</script>
