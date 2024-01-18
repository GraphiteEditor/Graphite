<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import { type RadioEntries, type RadioEntryData } from "@graphite/wasm-communication/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@graphite/components/widgets/labels/TextLabel.svelte";

	const dispatch = createEventDispatcher<{ selectedIndex: number }>();

	export let entries: RadioEntries;
	export let selectedIndex: number | undefined = undefined;
	export let disabled = false;
	export let minWidth = 0;

	$: mixed = selectedIndex === undefined && !disabled;

	function handleEntryClick(radioEntryData: RadioEntryData) {
		const index = entries.indexOf(radioEntryData);
		dispatch("selectedIndex", index);

		radioEntryData.action?.();
	}
</script>

<LayoutRow class="radio-input" classes={{ disabled }} styles={{ "min-width": minWidth > 0 ? `${minWidth}px` : "" }}>
	{#each entries as entry, index}
		<button class:active={index === selectedIndex} class:mixed class:disabled on:click={() => handleEntryClick(entry)} title={entry.tooltip} tabindex={index === selectedIndex ? -1 : 0} {disabled}>
			{#if entry.icon}
				<IconLabel icon={entry.icon} />
			{/if}
			{#if entry.label}
				<TextLabel italic={mixed}>{entry.label}</TextLabel>
			{/if}
		</button>
	{/each}
</LayoutRow>

<style lang="scss" global>
	.radio-input {
		button {
			background: var(--color-5-dullgray);
			fill: var(--color-e-nearwhite);
			height: 24px;
			margin: 0;
			padding: 0;
			border: none;
			display: flex;
			align-items: center;
			justify-content: center;
			// `min-width: fit-content` and `flex: 1 1 0` together allow us to occupy space such that we're always at least the content width,
			// but if the container is set wider, we distribute the space evenly (so buttons with short and long labels would have equal widths).
			min-width: fit-content;
			flex: 1 1 0;

			&.mixed {
				background: var(--color-4-dimgray);
			}

			&:hover {
				background: var(--color-6-lowergray);
				color: var(--color-f-white);

				svg {
					fill: var(--color-f-white);
				}
			}

			&.active {
				background: var(--color-e-nearwhite);
				color: var(--color-2-mildblack);

				svg {
					fill: var(--color-2-mildblack);
				}
			}

			&.disabled {
				background: var(--color-4-dimgray);
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

			& + button {
				margin-left: 1px;
			}

			&:first-of-type {
				border-radius: 2px 0 0 2px;
			}

			&:last-of-type {
				border-radius: 0 2px 2px 0;
			}

			.icon-label {
				margin: 0 4px;

				+ .text-label {
					margin-left: 0;
				}
			}

			.text-label {
				margin: 0 8px;
				overflow: hidden;
				flex: 0 0 auto;
			}
		}

		&.combined-before button:first-of-type,
		&.combined-after button:last-of-type {
			border-radius: 0;
		}
	}
</style>
