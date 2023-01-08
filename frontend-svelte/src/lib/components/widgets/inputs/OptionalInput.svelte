<script lang="ts">
	import { createEventDispatcher } from "svelte";

	import { type IconName } from "@/utility-functions/icons";

	import LayoutRow from "$lib/components/layout/LayoutRow.svelte";
	import CheckboxInput from "$lib/components/widgets/inputs/CheckboxInput.svelte";

	// emits: ["update:checked"],

	export let checked: boolean;
	export let disabled = false;
	export let icon: IconName = "Checkmark";
	export let tooltip: string | undefined = undefined;
</script>

<LayoutRow class="optional-input" classes={{ disabled }}>
	<CheckboxInput {checked} {disabled} on:input={(e) => createEventDispatcher("update:checked", e.target.checked)} {icon} {tooltip} />
</LayoutRow>

<style lang="scss" global>
	.optional-input {
		flex-grow: 0;

		label {
			align-items: center;
			justify-content: center;
			white-space: nowrap;
			width: 24px;
			height: 24px;
			border: 1px solid var(--color-5-dullgray);
			border-radius: 2px 0 0 2px;
			box-sizing: border-box;
		}

		&.disabled label {
			border: 1px solid var(--color-4-dimgray);
		}
	}
</style>
