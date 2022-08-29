<template>
	<div class="pivot-assist">
		<button @click="setPosition('TopLeft')" class="row-1 col-1" :class="{ active: position === 'TopLeft' }"></button>
		<button @click="setPosition('TopCenter')" class="row-1 col-2" :class="{ active: position === 'TopCenter' }"><div></div></button>
		<button @click="setPosition('TopRight')" class="row-1 col-3" :class="{ active: position === 'TopRight' }"><div></div></button>
		<button @click="setPosition('CenterLeft')" class="row-2 col-1" :class="{ active: position === 'CenterLeft' }"><div></div></button>
		<button @click="setPosition('Center')" class="row-2 col-2" :class="{ active: position === 'Center' }"><div></div></button>
		<button @click="setPosition('CenterRight')" class="row-2 col-3" :class="{ active: position === 'CenterRight' }"><div></div></button>
		<button @click="setPosition('BottomLeft')" class="row-3 col-1" :class="{ active: position === 'BottomLeft' }"><div></div></button>
		<button @click="setPosition('BottomCenter')" class="row-3 col-2" :class="{ active: position === 'BottomCenter' }"><div></div></button>
		<button @click="setPosition('BottomRight')" class="row-3 col-3" :class="{ active: position === 'BottomRight' }"><div></div></button>
	</div>
</template>

<style lang="scss">
.pivot-assist {
	position: relative;
	flex: 0 0 auto;
	width: 24px;
	height: 24px;

	button {
		position: absolute;
		width: 5px;
		height: 5px;
		margin: 0;
		padding: 0;
		outline: none;
		background: none;
		border: 1px solid var(--color-7-middlegray);

		&:hover {
			border-color: transparent;
			background: var(--color-6-lowergray);
		}

		&.active {
			border-color: transparent;
			background: var(--color-f-white);
		}

		&.col-1::before,
		&.col-2::before {
			content: "";
			pointer-events: none;
			width: 2px;
			height: 0;
			border-top: 1px solid var(--color-7-middlegray);
			position: absolute;
			top: 1px;
			right: -3px;
		}

		&.row-1::after,
		&.row-2::after {
			content: "";
			pointer-events: none;
			width: 0;
			height: 2px;
			border-left: 1px solid var(--color-7-middlegray);
			position: absolute;
			bottom: -3px;
			right: 1px;
		}

		&.row-1 {
			top: 3px;
		}
		&.col-1 {
			left: 3px;
		}

		&.row-2 {
			top: 10px;
		}
		&.col-2 {
			left: 10px;
		}

		&.row-3 {
			top: 17px;
		}
		&.col-3 {
			left: 17px;
		}

		// Click targets that extend 1px beyond the borders of each square
		div {
			width: 100%;
			height: 100%;
			padding: 2px;
			margin: -2px;
		}
	}
}
</style>

<script lang="ts">
import { defineComponent, type PropType } from "vue";

import { type PivotPosition } from "@/wasm-communication/messages";

export default defineComponent({
	emits: ["update:position"],
	props: {
		position: { type: String as PropType<string>, required: true },
	},
	methods: {
		setPosition(newPosition: PivotPosition) {
			this.$emit("update:position", newPosition);
		},
	},
});
</script>
