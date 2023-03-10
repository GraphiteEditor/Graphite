<script lang="ts">
	import type { IconName } from "@/utility-functions/icons";

	import IconLabel from "@/components/widgets/labels/IconLabel.svelte";
	import TextLabel from "@/components/widgets/labels/TextLabel.svelte";

	export let label: string;
	export let icon: IconName | undefined = undefined;
	export let emphasized = false;
	export let minWidth = 0;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;
	export let sharpRightCorners = false;

	// Callbacks
	// TODO: Replace this with an event binding (and on other components that do this)
	export let action: (e: MouseEvent) => void;
</script>

<button
	class="text-button"
	class:emphasized
	class:disabled
	class:sharp-right-corners={sharpRightCorners}
	style:min-width={minWidth > 0 ? `${minWidth}px` : undefined}
	title={tooltip}
	data-emphasized={emphasized || undefined}
	data-disabled={disabled || undefined}
	data-text-button
	tabindex={disabled ? -1 : 0}
	on:click={action}
>
	{#if icon}
		<IconLabel {icon} />
	{/if}
	<TextLabel>{label}</TextLabel>
</button>

<style lang="scss" global>
	.text-button {
		display: flex;
		justify-content: center;
		align-items: center;
		flex: 0 0 auto;
		height: 24px;
		margin: 0;
		padding: 0 8px;
		box-sizing: border-box;
		border: none;
		border-radius: 2px;
		background: var(--button-background-color);
		color: var(--button-text-color);
		--button-background-color: var(--color-5-dullgray);
		--button-text-color: var(--color-e-nearwhite);

		&:hover {
			--button-background-color: var(--color-6-lowergray);
			--button-text-color: var(--color-f-white);
		}

		&.disabled {
			--button-background-color: var(--color-4-dimgray);
			--button-text-color: var(--color-8-uppergray);
		}

		&.emphasized {
			--button-background-color: var(--color-e-nearwhite);
			--button-text-color: var(--color-2-mildblack);

			&:hover {
				--button-background-color: var(--color-f-white);
			}

			&.disabled {
				--button-background-color: var(--color-8-uppergray);
			}
		}

		& + .text-button {
			margin-left: 8px;
		}

		.icon-label {
			position: relative;
			left: -4px;
		}

		.text-label {
			overflow: hidden;
		}
	}
</style>
