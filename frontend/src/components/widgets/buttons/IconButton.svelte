<script lang="ts">
	import { type IconName, type IconSize } from "@graphite/icons";
	import type { ActionShortcut } from "@graphite/messages";

	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	// Content
	export let icon: IconName;
	export let hoverIcon: IconName | undefined = undefined;
	export let size: IconSize;
	export let disabled = false;
	// Styling
	export let emphasized = false;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<button
	class={`icon-button size-${size} ${className} ${extraClasses}`.trim()}
	class:hover-icon={hoverIcon && !disabled}
	class:disabled
	class:emphasized
	on:click={action}
	{disabled}
	data-tooltip-label={tooltipLabel}
	data-tooltip-description={tooltipDescription}
	data-tooltip-shortcut={tooltipShortcut?.shortcut ? JSON.stringify(tooltipShortcut.shortcut) : undefined}
	tabindex={emphasized ? -1 : 0}
	{...$$restProps}
>
	<IconLabel {icon} />
	{#if hoverIcon && !disabled}
		<IconLabel icon={hoverIcon} />
	{/if}
</button>

<style lang="scss" global>
	.icon-button {
		display: flex;
		justify-content: center;
		align-items: center;
		flex: 0 0 auto;
		margin: 0;
		padding: 0;
		border: none;
		border-radius: 2px;
		background: none;

		svg {
			fill: var(--color-e-nearwhite);
		}

		// The `where` pseudo-class does not contribtue to specificity
		& + :where(.icon-button) {
			margin-left: 0;
		}

		&:hover {
			background: var(--color-5-dullgray);
		}

		&.hover-icon {
			&:not(:hover) .icon-label:nth-of-type(2) {
				display: none;
			}

			&:hover .icon-label:nth-of-type(1) {
				display: none;
			}
		}

		&.disabled {
			background: none;

			svg {
				fill: var(--color-8-uppergray);
			}
		}

		&.emphasized {
			background: var(--color-e-nearwhite);

			svg {
				fill: var(--color-2-mildblack);
			}
		}

		&.size-12 {
			width: 12px;
			height: 12px;
		}

		&.size-16 {
			width: 16px;
			height: 16px;
		}

		&.size-24 {
			width: 24px;
			height: 24px;
		}

		&.size-32 {
			width: 32px;
			height: 32px;
		}
	}
</style>
