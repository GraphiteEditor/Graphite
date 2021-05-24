<template>
	<button class="icon-button" :class="`size-${String(size)}`">
		<slot></slot>
	</button>
</template>

<style lang="scss">
.icon-button {
	display: inline-block;
	flex: 0 0 auto;
	padding: 0;
	outline: none;
	border: none;
	border-radius: 2px;
	background: none;
	vertical-align: top;
	fill: var(--color-e-nearwhite);
	width: 16px;
	height: 16px;

	// The `where` pseduo-class does not contribtue to specificity
	& + :where(.icon-button) {
		margin-left: 0;
	}

	&:hover {
		background: var(--color-6-lowergray);
		color: var(--color-f-white);
		fill: var(--color-f-white);
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
