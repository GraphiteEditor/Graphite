<template>
	<div class="shelf-item" :class="{ active: active }">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.shelf-item {
	padding: 4px;
	border-radius: 4px;

	&:hover {
		background: #666;
	}

	&.active {
		background: #3194d6;
	}

	svg {
		width: 24px;
		height: 24px;
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
