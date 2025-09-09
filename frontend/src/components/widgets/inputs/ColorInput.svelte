<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { FillChoice, MenuDirection } from "@graphite/messages";
	import { Color, contrastingOutlineFactor, Gradient } from "@graphite/messages";

	import ColorPicker from "@graphite/components/floating-menus/ColorPicker.svelte";
	import LayoutCol from "@graphite/components/layout/LayoutCol.svelte";

	const dispatch = createEventDispatcher<{ value: FillChoice; startHistoryTransaction: undefined }>();

	let open = false;

	export let value: FillChoice;
	export let disabled = false;
	export let narrow = false;
	export let allowNone = false;
	export let menuDirection: MenuDirection = "Bottom";
	// export let allowTransparency = false; // TODO: Implement
	export let tooltip: string | undefined = undefined;

	$: outlineFactor = contrastingOutlineFactor(value, ["--color-1-nearblack", "--color-3-darkgray"], 0.01);
	$: outlined = outlineFactor > 0.0001;
	$: chosenGradient = value instanceof Gradient ? value.toLinearGradientCSS() : `linear-gradient(${value.toHexOptionalAlpha()}, ${value.toHexOptionalAlpha()})`;
	$: none = value instanceof Color ? value.none : false;
	$: transparency = value instanceof Gradient ? value.stops.some((stop) => stop.color.alpha < 1) : value.alpha < 1;
</script>

<LayoutCol class="color-button" classes={{ open, disabled, narrow, none, transparency, outlined, "direction-top": menuDirection === "Top" }} {tooltip}>
	<button style:--chosen-gradient={chosenGradient} style:--outline-amount={outlineFactor} on:click={() => (open = true)} tabindex="0" data-floating-menu-spawner>
		<!-- {#if disabled && value instanceof Color && !value.none}
			<TextLabel>sRGB</TextLabel>
		{/if} -->
	</button>
	<ColorPicker
		{open}
		{disabled}
		colorOrGradient={value}
		direction={menuDirection || "Bottom"}
		on:open={({ detail }) => (open = detail)}
		on:colorOrGradient={({ detail }) => {
			value = detail;
			dispatch("value", detail);
		}}
		on:startHistoryTransaction={() => {
			// This event is sent to the backend so it knows to start a transaction for the history system. See discussion for some explanation:
			// <https://github.com/GraphiteEditor/Graphite/pull/1584#discussion_r1477592483>
			dispatch("startHistoryTransaction");
		}}
		{allowNone}
	/>
</LayoutCol>

<style lang="scss" global>
	.color-button {
		position: relative;
		min-width: 80px;

		&.narrow.narrow {
			--widget-height: 20px;
		}

		> button {
			border: none;
			border-radius: 2px;
			padding: 0;
			margin: 0;
			width: 100%;
			height: 100%;
			overflow: hidden;
			position: relative;

			&::before {
				content: "";
				position: absolute;
				top: 0;
				bottom: 0;
				left: 0;
				right: 0;
				background: var(--chosen-gradient);
			}

			.text-label {
				background: var(--color-5-dullgray);
				font-size: 10px;
				line-height: 12px;
				height: 12px;
				border-radius: 0 0 0 2px;
				padding-right: 4px;
				padding-left: 4px;
				position: absolute;
				right: 0;
				top: 0;
			}
		}

		&.outlined > button::after {
			content: "";
			position: absolute;
			top: 0;
			bottom: 0;
			left: 0;
			right: 0;
			box-shadow: inset 0 0 0 1px rgba(var(--color-5-dullgray-rgb), var(--outline-amount));
		}

		&.transparency > button {
			background-image: var(--color-transparent-checkered-background);
			background-size: var(--color-transparent-checkered-background-size);
			background-position: var(--color-transparent-checkered-background-position);
			background-repeat: var(--color-transparent-checkered-background-repeat);
		}

		&:not(.disabled).none > button {
			background: var(--color-none);
			background-repeat: var(--color-none-repeat);
			background-position: var(--color-none-position);
			background-size: var(--color-none-size-24px);
			background-image: var(--color-none-image-24px);
		}

		&.disabled.none > button::after {
			content: "";
			position: absolute;
			top: 0;
			bottom: 0;
			left: 0;
			right: 0;
			background: var(--color-4-dimgray);
		}

		&:not(.disabled):hover > button .text-label,
		&:not(.disabled).open > button .text-label {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);
		}

		&.disabled > button .text-label {
			background: var(--color-4-dimgray);
			color: var(--color-8-uppergray);
		}

		> .floating-menu {
			left: 50%;
			bottom: 0;
		}

		&.direction-top > .floating-menu {
			bottom: 100%;
		}
	}
</style>
