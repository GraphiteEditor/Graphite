<script lang="ts">
	import type { SvelteHTMLElements } from "svelte/elements";

	import { type IconName, type IconSize } from "@graphite/utility-functions/icons";

	import IconLabel from "@graphite/components/widgets/labels/IconLabel.svelte";

	type ButtonHTMLElementProps = SvelteHTMLElements["button"];

	type Props = {
		class?: string;
		classes?: Record<string, boolean>;
		icon: IconName;
		hoverIcon?: IconName | undefined;
		size: IconSize;
		disabled?: boolean;
		active?: boolean;
		tooltip?: string | undefined;
	} & ButtonHTMLElementProps;

	let { class: className = "", classes = {}, icon, hoverIcon = undefined, size, disabled = false, active = false, tooltip = undefined, ...rest }: Props = $props();

	let extraClasses = $derived(
		Object.entries(classes)
			.flatMap(([className, stateName]) => (stateName ? [className] : []))
			.join(" "),
	);
</script>

<button
	class={`icon-button size-${size} ${className} ${extraClasses}`.trim()}
	class:hover-icon={hoverIcon && !disabled}
	class:disabled
	class:active
	{disabled}
	title={tooltip}
	tabindex={active ? -1 : 0}
	{...rest}
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
		& + :where(:global(.icon-button)) {
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

		&.active {
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
