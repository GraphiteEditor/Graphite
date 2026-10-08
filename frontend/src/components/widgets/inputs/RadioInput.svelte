<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "/src/components/widgets/labels/TextLabel.svelte";
	import type { RadioEntryData } from "/wrapper/pkg/graphite_wasm_wrapper";

	const dispatch = createEventDispatcher<{ selectedIndex: number }>();

	// Content
	export let selectedIndex: number | undefined = undefined;
	export let disabled = false;
	// Children
	export let entries: RadioEntryData[];
	// Styling
	export let narrow = false;
	// Sizing
	export let autoWidth = false;
	export let minWidth = 0;
	export let maxWidth = 0;

	$: mixed = selectedIndex === undefined && !disabled;

	function handleEntryClick(radioEntryData: RadioEntryData) {
		const index = entries.indexOf(radioEntryData);
		dispatch("selectedIndex", index);
	}
</script>

<LayoutRow
	class="radio-input"
	classes={{ disabled, narrow, mixed, "auto-width": autoWidth }}
	styles={{
		...(minWidth > 0 ? { "min-width": `${minWidth}px` } : {}),
		...(maxWidth > 0 ? { "max-width": `${maxWidth}px` } : {}),
	}}
>
	{#each entries as entry, index}
		<button
			class:active={!mixed ? index === selectedIndex : undefined}
			on:click={() => handleEntryClick(entry)}
			data-tooltip-label={entry.tooltipLabel}
			data-tooltip-description={entry.tooltipDescription}
			data-tooltip-shortcut={entry.tooltipShortcut?.shortcut ? JSON.stringify(entry.tooltipShortcut.shortcut) : undefined}
			tabindex={index === selectedIndex ? -1 : 0}
			{disabled}
		>
			{#if entry.icon}
				<IconLabel icon={entry.icon} />
			{/if}
			{#if entry.label}
				<TextLabel>{entry.label}</TextLabel>
			{/if}
		</button>
	{/each}
</LayoutRow>

<style lang="scss">
	.radio-input {
		background: var(--color-4-dimgray);
		border-radius: 2px;
		--widget-height: 24px;
		height: var(--widget-height);
		// Insets the ends here rather than with wider end margins, which would widen every equal-width column
		padding: 0 1px;

		// Equal-width buttons, each as wide as the widest label until the radio is given more or less room to divide evenly
		&.radio-input {
			display: grid;
			grid-auto-flow: column;
			grid-auto-columns: minmax(24px, 1fr);
			min-width: min-content;
		}

		// Icons, and labels when the radio is auto-width, stay whole rather than being cut off
		&:not(:has(.text-label)),
		&.auto-width {
			grid-auto-columns: minmax(min-content, 1fr);
		}

		button {
			background: var(--color-4-dimgray);
			fill: var(--color-e-nearwhite);
			border-radius: 2px;
			height: 20px;
			padding: 0;
			margin: 2px 1px;
			border: none;
			display: flex;
			align-items: center;
			justify-content: center;

			&.active {
				background: var(--color-e-nearwhite);
				color: var(--color-2-mildblack);

				svg {
					fill: var(--color-2-mildblack);
				}
			}

			.icon-label {
				margin: 2px;

				+ .text-label {
					margin-left: 0;
				}
			}

			.text-label {
				margin: 0 8px;
				overflow: hidden;
				flex: 0 1 auto;
				text-overflow: ellipsis;
			}
		}

		&:not(.disabled) button:not(.active):hover {
			background: var(--color-6-lowergray);
			color: var(--color-f-white);

			svg {
				fill: var(--color-f-white);
			}
		}

		&.disabled button {
			color: var(--color-8-uppergray);

			svg {
				fill: var(--color-8-uppergray);
			}

			&.active {
				background: var(--color-8-uppergray);
				color: var(--color-2-mildblack);

				svg {
					fill: var(--color-2-mildblack);
				}
			}
		}

		&.narrow.narrow {
			--widget-height: 20px;
			height: var(--widget-height);

			button {
				height: 16px;
			}
		}

		&.mixed {
			button:not(:hover),
			&.disabled button:hover {
				background: var(--color-5-dullgray);
			}
		}
	}
</style>
