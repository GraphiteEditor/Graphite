<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { PivotPosition } from "@graphite/wasm-communication/messages";

	const dispatch = createEventDispatcher<{ position: PivotPosition }>();

	export let position: string;
	export let disabled = false;

	function setPosition(newPosition: PivotPosition) {
		dispatch("position", newPosition);
	}
</script>

<div class="pivot-input" class:disabled>
	<button on:click={() => setPosition("TopLeft")} class="row-1 col-1" class:active={position === "TopLeft"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("TopCenter")} class="row-1 col-2" class:active={position === "TopCenter"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("TopRight")} class="row-1 col-3" class:active={position === "TopRight"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("CenterLeft")} class="row-2 col-1" class:active={position === "CenterLeft"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("Center")} class="row-2 col-2" class:active={position === "Center"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("CenterRight")} class="row-2 col-3" class:active={position === "CenterRight"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("BottomLeft")} class="row-3 col-1" class:active={position === "BottomLeft"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("BottomCenter")} class="row-3 col-2" class:active={position === "BottomCenter"} tabindex="-1" {disabled}><div /></button>
	<button on:click={() => setPosition("BottomRight")} class="row-3 col-3" class:active={position === "BottomRight"} tabindex="-1" {disabled}><div /></button>
</div>

<style lang="scss" global>
	.pivot-input {
		position: relative;
		flex: 0 0 auto;
		width: 24px;
		height: 24px;
		--pivot-border-color: var(--color-5-dullgray);
		--pivot-fill-active: var(--color-e-nearwhite);

		button {
			position: absolute;
			width: 5px;
			height: 5px;
			margin: 0;
			padding: 0;
			background: var(--color-1-nearblack);
			border: 1px solid var(--pivot-border-color);

			&.active {
				border-color: transparent;
				background: var(--pivot-fill-active);
			}

			&.col-1::before,
			&.col-2::before {
				content: "";
				pointer-events: none;
				width: 2px;
				height: 0;
				border-top: 1px solid var(--pivot-border-color);
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
				border-left: 1px solid var(--pivot-border-color);
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

		&:not(.disabled) button:not(.active):hover {
			border-color: transparent;
			background: var(--color-6-lowergray);
		}

		&.disabled button {
			--pivot-border-color: var(--color-4-dimgray);
			--pivot-fill-active: var(--color-8-uppergray);
		}
	}
</style>
