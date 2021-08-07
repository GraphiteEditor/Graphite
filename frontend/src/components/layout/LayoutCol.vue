<template>
	<div :class="['layout-col']">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.layout-col {
	display: flex;
	flex-direction: column;
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
