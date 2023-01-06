<template>
	<LayoutRow class="window-buttons mac">
		<div class="close" title="Close"></div>
		<div class="minimize" title="Minimize"></div>
		<div class="zoom" title="Zoom"></div>
	</LayoutRow>
</template>

<style lang="scss">
.window-buttons.mac {
	flex: 0 0 auto;
	align-items: center;
	margin: 0 8px;

	div {
		flex: 0 0 auto;
		align-items: center;
		width: 11px;
		height: 11px;
		border-radius: 50%;

		& + div {
			margin-left: 8px;
		}

		&.close {
			background: #ff5a52;
		}

		&.minimize {
			background: #e6c029;
		}

		&.zoom {
			background: #54c22b;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import LayoutRow from "@/components/layout/LayoutRow.vue";

export default defineComponent({
	props: {
		maximized: { type: Boolean as PropType<boolean>, default: false },
	},
	components: { LayoutRow },
});
</script>
