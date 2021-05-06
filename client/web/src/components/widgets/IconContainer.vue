<template>
	<div class="icon-container" :class="[`size-${String(size)}`, gapAfter && 'gap-after']">
		<slot></slot>
	</div>
</template>

<style lang="scss">
.icon-container {
	display: inline-block;
	flex: 0 0 auto;
	fill: #ddd;
	vertical-align: top;
	width: 16px;
	height: 16px;

	&:not(.gap-after) + .icon-container {
		margin-left: 0;
	}

	&.size-10 {
		width: 10px;
		height: 10px;
	}

	&.size-12 {
		width: 12px;
		height: 12px;
	}

	&.size-16 {
		width: 16px;
		height: 16px;
	}

	&.size-24 {
		width: 24px;
		height: 24px;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	props: {
		size: { type: Number, required: true },
		gapAfter: { type: Boolean, default: false },
	},
});
</script>
