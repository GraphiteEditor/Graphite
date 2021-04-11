<template>
	<button class="icon-button" :class="[`size-${String(size)}`, gapAfter && 'gap-after']">
		<slot></slot>
	</button>
</template>

<style lang="scss">
.icon-button {
	display: inline-block;
	flex: 0 0 auto;
	outline: none;
	border: none;
	padding: 0;
	width: 16px;
	height: 16px;
	background: none;
	font-weight: bold;
	font-size: 10px;
	border-radius: 2px;
	color: #ddd;
	fill: #ddd;

	&:not(.gap-after) + .icon-button {
		margin-left: 0;
	}

	&:hover {
		background: #666;
		color: white;
		fill: white;
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
