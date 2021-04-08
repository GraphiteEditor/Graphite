<template>
	<div :class="['layout-row']">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.layout-row {
	display: flex;
	flex-direction: row;
	flex-grow: 1;

	.spacer {
		flex: 1 1 100%;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({});
</script>
