<template>
	<div class="shelf-item" :class="{ active: active }">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.shelf-item {
	padding: 4px;
	border-radius: 2px;

	&:hover {
		background: var(--color-6-lowergray);
	}

	&.active {
		background: var(--color-accent);
	}

	svg {
		width: 24px;
		height: 24px;
		vertical-align: top;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	props: {
		active: { type: Boolean, default: false },
	},
});
</script>
