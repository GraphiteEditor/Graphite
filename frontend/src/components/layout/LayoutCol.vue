<template>
	<div class="layout-col" :class="{ 'scrollable-x': scrollableX, 'scrollable-y': scrollableY }" :data-scrollable-x="scrollableX || null" :data-scrollable-y="scrollableY || null">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.layout-col {
	display: flex;
	flex-direction: column;
	flex-grow: 1;
	min-width: 0;
	min-height: 0;

	.spacer {
		flex: 1 1 100%;
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

export default defineComponent({
	props: {
		scrollableX: { type: Boolean as PropType<boolean>, default: false },
		scrollableY: { type: Boolean as PropType<boolean>, default: false },
	},
});
</script>
