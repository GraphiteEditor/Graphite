<template>
	<div class="number-input">
		<button class="arrow left"></button>
		<button class="arrow right"></button>
		<input type="text" spellcheck="false" :value="`${value}${unit}`" />
	</div>
</template>

<style lang="scss">
.number-input {
	width: 64px;
	height: 24px;
	position: relative;
	border-radius: 2px;
	background: var(--color-1-nearblack);
	overflow: hidden;

	input {
		width: calc(100% - 8px);
		margin: 0 4px;
		outline: none;
		border: none;
		background: none;
		padding: 0;
		line-height: 24px;
		color: var(--color-e-nearwhite);
		font-size: inherit;
		text-align: center;
		font-family: inherit;

		&::selection {
			background: var(--color-accent);
		}
	}

	&:not(:hover) .arrow {
		display: none;
	}

	.arrow {
		position: absolute;
		top: 0;
		outline: none;
		border: none;
		background: none;
		padding: 9px 0;

		&:hover {
			background: var(--color-6-lowergray);

			&.right::before {
				border-color: transparent transparent transparent var(--color-f-white);
			}

			&.left::after {
				border-color: transparent var(--color-f-white) transparent transparent;
			}
		}

		&.right {
			right: 0;
			padding-left: 7px;
			padding-right: 6px;

			&::before {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 0 3px 3px;
				border-color: transparent transparent transparent var(--color-e-nearwhite);
				display: block;
			}
		}

		&.left {
			left: 0;
			padding-left: 6px;
			padding-right: 7px;

			&::after {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 3px 3px 0;
				border-color: transparent var(--color-e-nearwhite) transparent transparent;
				display: block;
			}
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {
		value: { type: Number, required: true },
		unit: { type: String, default: "" },
	},
});
</script>
