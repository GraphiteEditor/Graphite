<template>
	<div class="layout-row" :class="{ 'scrollable-x': scrollableX, 'scrollable-y': scrollableY }">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.layout-row {
	display: flex;
	flex-direction: row;
	flex-grow: 1;
	min-width: 0;
	min-height: 0;

	.spacer {
		flex: 1 1 100%;
	}
}
</style>

<script lang="ts">
import { defineComponent, PropType } from "vue";

export default defineComponent({
	props: {
		scrollableX: { type: Boolean as PropType<boolean>, default: false },
		scrollableY: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
