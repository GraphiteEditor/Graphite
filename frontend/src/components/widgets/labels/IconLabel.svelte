<script lang="ts">
	import { type IconName, ICONS, ICON_SVG_STRINGS } from "@graphite/icons";
	import type { ActionShortcut } from "@graphite/messages";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let iconSizeOverride: number | undefined = undefined;

	// Content
	export let icon: IconName;
	export let disabled = false;
	// Tooltips
	export let tooltipLabel: string | undefined = undefined;
	export let tooltipDescription: string | undefined = undefined;
	export let tooltipShortcut: ActionShortcut | undefined = undefined;

	$: iconSizeClass = ((icon: IconName) => {
		const iconData = ICONS[icon];
		if (!iconData) {
			// eslint-disable-next-line no-console
			console.warn(`Icon "${icon}" does not exist.`);
			return "size-24";
		}
		if (iconData.size === undefined) return "";
		return `size-${iconSizeOverride || iconData.size}`;
	})(icon);
	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<LayoutRow class={`icon-label ${iconSizeClass} ${className} ${extraClasses}`.trim()} classes={{ disabled }} {tooltipLabel} {tooltipDescription} {tooltipShortcut}>
	{@html ICON_SVG_STRINGS[icon] || "�"}
</LayoutRow>

<style lang="scss" global>
	.icon-label {
		flex: 0 0 auto;
		fill: var(--color-e-nearwhite);

		&.disabled {
			fill: var(--color-8-uppergray);
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
	}
</style>
