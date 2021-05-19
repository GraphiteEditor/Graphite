<template>
	<div class="popover-color-picker" @click="notImplemented">
		<div class="color-picker">
			<div class="selection-circle"></div>
		</div>
		<div class="hue-picker">
			<div class="selection-pincers"></div>
		</div>
		<div class="opacity-picker">
			<div class="selection-pincers"></div>
		</div>
	</div>
</template>

<style lang="scss">
.popover-color-picker {
	display: flex;

	.color-picker {
		--hue: #ff0000;
		width: 256px;
		background-blend-mode: multiply;
		background: linear-gradient(to bottom, #ffffff, #000000), linear-gradient(to right, #ffffff, var(--hue));
		position: relative;
	}

	.color-picker,
	.hue-picker,
	.opacity-picker {
		height: 256px;
		position: relative;
		overflow: hidden;
	}

	.hue-picker,
	.opacity-picker {
		width: 24px;
		margin-left: 8px;
		position: relative;
	}

	.hue-picker {
		background-blend-mode: screen;
		background: linear-gradient(to top, #ff0000ff 16.666%, #ff000000 33.333%, #ff000000 66.666%, #ff0000ff 83.333%),
			linear-gradient(to top, #00ff0000 0%, #00ff00ff 16.666%, #00ff00ff 50%, #00ff0000 66.666%), linear-gradient(to top, #0000ff00 33.333%, #0000ffff 50%, #0000ffff 83.333%, #0000ff00 100%);
	}

	.opacity-picker {
		background: linear-gradient(to bottom, #ff0000, transparent);

		&::before {
			content: "";
			display: block;
			width: 100%;
			height: 100%;
			background: linear-gradient(45deg, #ccc 25%, transparent 25%, transparent 75%, #ccc 75%), linear-gradient(45deg, #ccc 25%, transparent 25%, transparent 75%, #ccc 75%),
				linear-gradient(#fff, #fff);
			background-size: 16px 16px;
			background-position: 0 0, 8px 8px;
			position: relative;
			z-index: -1;
		}
	}

	.selection-circle {
		position: absolute;
		left: 100%;
		top: 0%;
		width: 0;
		height: 0;

		&::after {
			content: "";
			display: block;
			position: relative;
			left: -6px;
			top: -6px;
			width: 12px;
			height: 12px;
			border-radius: 50%;
			border: 2px solid white;
			box-sizing: border-box;
			mix-blend-mode: difference;
		}
	}

	.selection-pincers {
		position: absolute;
		top: 0%;
		width: 100%;
		height: 0;

		&::before {
			content: "";
			position: absolute;
			top: -4px;
			left: 0;
			border-style: solid;
			border-width: 4px 0 4px 4px;
			border-color: transparent transparent transparent black;
		}

		&::after {
			content: "";
			position: absolute;
			top: -4px;
			right: 0;
			border-style: solid;
			border-width: 4px 4px 4px 0;
			border-color: transparent black transparent transparent;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

export default defineComponent({
	components: {},
	props: {},
	methods: {
		notImplemented() {
			alert("Color picker is not functional yet");
		},
	},
});
</script>
