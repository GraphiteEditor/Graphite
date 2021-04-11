<template>
	<div class="number-input">
		<button class="arrow left"></button>
		<button class="arrow right"></button>
		<input type="text" spellcheck="false" value="25%" />
	</div>
</template>

<style lang="scss">
.number-input {
	width: 64px;
	height: 22px;
	position: relative;
	border: 1px solid #888;
	border-radius: 2px;
	background: #111;

	input {
		width: calc(100% - 8px);
		margin: 0 4px;
		outline: none;
		border: none;
		background: none;
		padding: 0;
		line-height: 22px;
		color: #ddd;
		font-size: 14px;
		text-align: center;
		font-family: inherit;

		&::selection {
			background: #3194d6;
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
		background: #333;

		&:hover {
			background: #555;
		}

		&.right {
			right: 0;
			padding: 8px 6px 8px 7px;

			&::before {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 0 3px 3px;
				border-color: transparent transparent transparent #ddd;
				display: block;
			}
		}

		&.left {
			left: 0;
			padding: 8px 7px 8px 6px;

			&::after {
				content: "";
				width: 0;
				height: 0;
				border-style: solid;
				border-width: 3px 3px 3px 0;
				border-color: transparent #ddd transparent transparent;
				display: block;
			}
		}
	}

}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {
	},
	props: {
	},
});
</script>
