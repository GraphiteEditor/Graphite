<template>
	<LayoutCol :class="'document'">
		<LayoutRow :class="'options-bar'">
			<div class="left side">
				<!-- <span class="label">Select</span>
				<div class="divider"></div> -->
			</div>
			<div class="spacer"></div>
			<div class="right side">
				<!-- <span class="label">Layer 1</span> -->
			</div>
		</LayoutRow>
		<LayoutRow :class="'toolbar-and-viewport'">
			<LayoutCol :class="'toolbar'"></LayoutCol>
			<LayoutCol :class="'viewport'">
				<div
					class="canvas"
					@mousedown="canvasMouseDown"
					@mouseup="canvasMouseUp"
					@mousemove="canvasMouseMove"
				>
					<svg></svg>
				</div>
			</LayoutCol>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.document {
	height: 100%;

	.options-bar {
		flex: 0 0 32px;

		.side {
			height: 100%;
			flex: 0 1 auto;
			display: flex;
			align-items: center;
			margin: 0 4px;

			> * {
				margin: 0 4px;
			}

			.label {
				white-space: nowrap;
				font-weight: bold;
			}

			.divider {
				width: 1px;
				height: 24px;
				background: #888;
			}
		}

		.spacer {
			flex: 1 1 100%;
		}
	}

	.toolbar-and-viewport {
		.toolbar {
			flex: 0 0 32px;
		}

		.viewport {
			flex: 1 1 100%;

			.canvas {
				background: #111;
				width: 100%;
				height: 100%;

				svg {
					width: 100%;
					height: 100%;
				}
			}

		}
}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";

const wasm = import("../../../wasm/pkg");

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
	},
	methods: {
		async canvasMouseDown(e: MouseEvent) {
			const { on_mouse_down } = await wasm;
			on_mouse_down(e.offsetX, e.offsetY, e.buttons);
		},
		async canvasMouseUp(e: MouseEvent) {
			const { on_mouse_up } = await wasm;
			on_mouse_up(e.offsetX, e.offsetY, e.buttons);
		},
		async canvasMouseMove(e: MouseEvent) {
			const { on_mouse_move } = await wasm;
			on_mouse_move(e.offsetX, e.offsetY);
		},
	},
});
</script>
