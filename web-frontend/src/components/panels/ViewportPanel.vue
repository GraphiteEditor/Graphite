<template>
	<LayoutCol :class="'viewport'">
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
		<LayoutRow :class="'tools-and-viewport'">
			<LayoutCol :class="'tools'"></LayoutCol>
			<LayoutCol :class="'viewport-container'"></LayoutCol>
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.viewport {
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

	.tools-and-viewport {
		.tools {
			flex: 0 0 32px;
		}

		.viewport-container {
			background: #111;
			flex: 1 1 100%;
		}
}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
	},
	props: {
	},
});
</script>
