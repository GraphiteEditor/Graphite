<script lang="ts">
	import { type IconName, ICONS, ICON_SVG_STRINGS } from "@graphite/utility-functions/icons";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};
	export let icon: IconName;
	export let iconSizeOverride: number | undefined = undefined;
	export let disabled = false;
	export let tooltip: string | undefined = undefined;

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

<LayoutRow class={`icon-label ${iconSizeClass} ${className} ${extraClasses}`.trim()} classes={{ disabled }} {tooltip}>
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
