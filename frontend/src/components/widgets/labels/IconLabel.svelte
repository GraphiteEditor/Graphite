<script lang="ts">
	import { type IconName, ICONS, ICON_SVG_STRINGS } from "@graphite/utility-functions/icons";

	import LayoutRow from "@graphite/components/layout/LayoutRow.svelte";

	type Props = {
		class?: string;
		classes?: Record<string, boolean>;
		icon: IconName;
		iconSizeOverride?: number | undefined;
		disabled?: boolean;
		tooltip?: string | undefined;
	};

	let { class: className = "", classes = {}, icon, iconSizeOverride = undefined, disabled = false, tooltip = undefined }: Props = $props();

	let iconSizeClass = $derived(
		((icon: IconName) => {
			const iconData = ICONS[icon];
			if (!iconData) {
				// eslint-disable-next-line no-console
				console.warn(`Icon "${icon}" does not exist.`);
				return "size-24";
			}
			if (iconData.size === undefined) return "";
			return `size-${iconSizeOverride || iconData.size}`;
		})(icon),
	);
	let extraClasses = $derived(
		Object.entries(classes)
			.flatMap(([className, stateName]) => (stateName ? [className] : []))
			.join(" "),
	);
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
