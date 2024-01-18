<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import type { IconName } from "@graphite/utility-functions/icons";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";
	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	const dispatch = createEventDispatcher<{ checked: boolean }>();

	export let checked = false;
	export let disabled = false;
	export let icon: IconName = "Checkmark";
	export let tooltip: string | undefined = undefined;

	let inputElement: HTMLInputElement | undefined;

	let id = String(Math.random()).substring(2);

	$: displayIcon = (!checked && icon === "Checkmark" ? "Empty12px" : icon) as IconName;

	export function isChecked() {
		return checked;
	}

	export function input(): HTMLInputElement | undefined {
		return inputElement;
	}

	function toggleCheckboxFromLabel(e: KeyboardEvent) {
		const target = (e.target || undefined) as HTMLLabelElement | undefined;
		const previousSibling = (target?.previousSibling || undefined) as HTMLInputElement | undefined;
		previousSibling?.click();
	}
</script>

<LayoutRow class="checkbox-input">
	<input type="checkbox" id={`checkbox-input-${id}`} {checked} on:change={(_) => dispatch("checked", inputElement?.checked)} {disabled} tabindex={disabled ? -1 : 0} bind:this={inputElement} />
	<label class:disabled class:checked for={`checkbox-input-${id}`} on:keydown={(e) => e.key === "Enter" && toggleCheckboxFromLabel(e)} title={tooltip}>
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
			&:hover .checkbox-box {
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
			&:hover .checkbox-box {
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
