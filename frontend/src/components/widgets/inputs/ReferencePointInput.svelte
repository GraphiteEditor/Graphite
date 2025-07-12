<script lang="ts">
	import type { ReferencePoint } from "@graphite/messages.svelte";

	type Props = {
		value: string;
		disabled?: boolean;
		onvalue?: (point: ReferencePoint) => void;
		tooltip?: string;
	};

	let { value, disabled = false, onvalue, tooltip }: Props = $props();

	function setValue(event: MouseEvent) {
		const element = event.target as HTMLDivElement;
		const button = element.parentElement;
		if (button instanceof HTMLButtonElement) {
			let position = button.dataset.position! as ReferencePoint;
			onvalue?.(position);
		}
	}
</script>

<div class="reference-point-input" class:disabled onclick={setValue} title={tooltip}>
	<button data-position="TopLeft" class="row-1 col-1" class:active={value === "TopLeft"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="TopCenter" class="row-1 col-2" class:active={value === "TopCenter"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="TopRight" class="row-1 col-3" class:active={value === "TopRight"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="CenterLeft" class="row-2 col-1" class:active={value === "CenterLeft"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="Center" class="row-2 col-2" class:active={value === "Center"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="CenterRight" class="row-2 col-3" class:active={value === "CenterRight"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="BottomLeft" class="row-3 col-1" class:active={value === "BottomLeft"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="BottomCenter" class="row-3 col-2" class:active={value === "BottomCenter"} tabindex="-1" {disabled}><div></div></button>
	<button data-position="BottomRight" class="row-3 col-3" class:active={value === "BottomRight"} tabindex="-1" {disabled}><div></div></button>
</div>

<style lang="scss" global>
	.reference-point-input {
		position: relative;
		flex: 0 0 auto;
		width: 24px;
		height: 24px;
		--reference-point-border-color: var(--color-5-dullgray);
		--reference-point-fill-active: var(--color-e-nearwhite);

		button {
			position: absolute;
			width: 5px;
			height: 5px;
			margin: 0;
			padding: 0;
			background: var(--color-1-nearblack);
			border: 1px solid var(--reference-point-border-color);

			&.active {
				border-color: transparent;
				background: var(--reference-point-fill-active);
			}

			&.col-1::before,
			&.col-2::before {
				content: "";
				pointer-events: none;
				width: 2px;
				height: 0;
				border-top: 1px solid var(--reference-point-border-color);
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
				border-left: 1px solid var(--reference-point-border-color);
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
			--reference-point-border-color: var(--color-4-dimgray);
			--reference-point-fill-active: var(--color-8-uppergray);
		}
	}
</style>
