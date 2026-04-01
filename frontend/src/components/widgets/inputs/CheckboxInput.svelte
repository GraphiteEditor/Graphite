<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import IconLabel from "/src/components/widgets/labels/IconLabel.svelte";
	import type { IconName } from "/src/icons";
	import type { ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

	const dispatch = createEventDispatcher<{ checked: boolean }>();
	const backupId = String(Math.random()).substring(2);

	// Content
	export let checked = false;
	export let icon: IconName | undefined = undefined;
	export let forLabel: bigint | undefined = undefined;
	export let disabled = false;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;

	let inputElement: HTMLInputElement | undefined;

	$: id = forLabel !== undefined ? String(forLabel) : backupId;
	$: displayIcon = !checked && (!icon || icon === "Checkmark") ? "Empty12px" : icon || "Checkmark";

	export function isChecked() {
		return checked;
	}

	export function input(): HTMLInputElement | undefined {
		return inputElement;
	}

	function toggleCheckboxFromLabel(e: KeyboardEvent) {
		const target = e.target instanceof HTMLLabelElement ? e.target : undefined;
		const previousSibling = target?.previousSibling instanceof HTMLInputElement ? target.previousSibling : undefined;
		previousSibling?.click();
	}
</script>

<LayoutRow class="checkbox-input">
	<input
		type="checkbox"
		id={`checkbox-input-${id}`}
		bind:checked
		on:change={(_) => dispatch("checked", inputElement?.checked || false)}
		{disabled}
		tabindex={disabled ? -1 : 0}
		bind:this={inputElement}
	/>
	<label
		class:disabled
		class:checked
		for={`checkbox-input-${id}`}
		on:keydown={(e) => e.key === "Enter" && toggleCheckboxFromLabel(e)}
		data-tooltip-label={tooltipLabel}
		data-tooltip-description={tooltipDescription}
		data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
	>
		<LayoutRow class="checkbox-box">
			<IconLabel icon={displayIcon} />
		</LayoutRow>
	</label>
</LayoutRow>

<style lang="scss" global>
	.checkbox-input {
		flex: 0 0 auto;
		align-items: center;

		input {
			// We can't use `display: none` because it must be visible to work as a tabbale input that accepts a space bar actuation
			width: 0;
			height: 0;
			margin: 0;
			opacity: 0;
			// Firefox weirdly applies a 2px border which causes this element to take up a 4x4 square of space, so this removes it from the flow to prevent it from offsetting the label
			position: absolute;
		}

		// Unchecked
		label {
			display: flex;
			height: 16px;
			// Provides rounded corners for the :focus outline
			border-radius: 2px;

			.checkbox-box {
				flex: 0 0 auto;
				background: var(--color-5-dullgray);
				padding: 2px;
				border-radius: 2px;

				.icon-label {
					fill: var(--color-8-uppergray);
				}
			}

			// Hovered while unchecked
			&:hover .checkbox-box,
			&.label-is-hovered .checkbox-box {
				background: var(--color-6-lowergray);
			}

			// Disabled while unchecked
			&.disabled .checkbox-box {
				background: var(--color-4-dimgray);
			}
		}

		// Checked
		input:checked + label {
			.checkbox-box {
				background: var(--color-e-nearwhite);

				.icon-label {
					fill: var(--color-2-mildblack);
				}
			}

			// Hovered while checked
			&:hover .checkbox-box,
			&.label-is-hovered .checkbox-box {
				background: var(--color-f-white);
			}

			// Disabled while checked
			&.disabled .checkbox-box {
				background: var(--color-8-uppergray);
			}
		}

		+ .text-label.text-label {
			margin-left: 8px;
		}
	}
</style>
