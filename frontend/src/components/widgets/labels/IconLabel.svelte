<script lang="ts">
	import LayoutRow from "/src/components/layout/LayoutRow.svelte";
	import { ICONS, ICON_SVG_STRINGS, ICON_16X16_NODES } from "/src/icons";
	import type { IconName } from "/src/icons";
	import type { ActionShortcut } from "/wrapper/pkg/graphite_wasm_wrapper";

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
	$: nodeIconClass = icon in ICON_16X16_NODES ? "node-icon" : "";
	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<LayoutRow class={`icon-label ${iconSizeClass} ${nodeIconClass} ${className} ${extraClasses}`.trim()} classes={{ disabled }} {tooltipLabel} {tooltipDescription} {tooltipShortcut}>
	{@html ICON_SVG_STRINGS[icon] || "�"}
</LayoutRow>

<style lang="scss">
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

		// Node icons color each shape by class, from the palette or a two-color mix of it
		&.node-icon {
			.color-7-middlegray {
				fill: var(--color-7-middlegray);
			}

			.color-8-uppergray {
				fill: var(--color-8-uppergray);
			}

			.color-9-palegray {
				fill: var(--color-9-palegray);
			}

			.color-a-softgray {
				fill: var(--color-a-softgray);
			}

			.color-artboard {
				fill: var(--color-data-artboard);
			}

			.color-c-brightgray {
				fill: var(--color-c-brightgray);
			}

			.color-color {
				fill: var(--color-data-color);
			}

			.color-color-dim {
				fill: var(--color-data-color-dim);
			}

			.color-e-nearwhite {
				fill: var(--color-e-nearwhite);
			}

			.color-general {
				fill: var(--color-data-general);
			}

			.color-general-dim {
				fill: var(--color-data-general-dim);
			}

			.color-gradient {
				fill: var(--color-data-gradient);
			}

			.color-gradient-dim {
				fill: var(--color-data-gradient-dim);
			}

			.color-graphic {
				fill: var(--color-data-graphic);
			}

			.color-graphic-dim {
				fill: var(--color-data-graphic-dim);
			}

			.color-number {
				fill: var(--color-data-number);
			}

			.color-number-dim {
				fill: var(--color-data-number-dim);
			}

			.color-raster {
				fill: var(--color-data-raster);
			}

			.color-raster-dim {
				fill: var(--color-data-raster-dim);
			}

			.color-typography {
				fill: var(--color-data-typography);
			}

			.color-typography-dim {
				fill: var(--color-data-typography-dim);
			}

			.color-vector {
				fill: var(--color-data-vector);
			}

			.color-vector-dim {
				fill: var(--color-data-vector-dim);
			}

			.color-mix-7-middlegray-8-uppergray-40 {
				fill: color-mix(in srgb, var(--color-7-middlegray), var(--color-8-uppergray) 40%);
			}

			.color-mix-8-uppergray-9-palegray-45 {
				fill: color-mix(in srgb, var(--color-8-uppergray), var(--color-9-palegray) 45%);
			}

			.color-mix-8-uppergray-9-palegray-65 {
				fill: color-mix(in srgb, var(--color-8-uppergray), var(--color-9-palegray) 65%);
			}

			.color-mix-8-uppergray-9-palegray-70 {
				fill: color-mix(in srgb, var(--color-8-uppergray), var(--color-9-palegray) 70%);
			}

			.color-mix-9-palegray-a-softgray-35 {
				fill: color-mix(in srgb, var(--color-9-palegray), var(--color-a-softgray) 35%);
			}

			.color-mix-9-palegray-a-softgray-95 {
				fill: color-mix(in srgb, var(--color-9-palegray), var(--color-a-softgray) 95%);
			}

			.color-mix-b-lightgray-c-brightgray-25 {
				fill: color-mix(in srgb, var(--color-b-lightgray), var(--color-c-brightgray) 25%);
			}

			.color-mix-b-lightgray-c-brightgray-65 {
				fill: color-mix(in srgb, var(--color-b-lightgray), var(--color-c-brightgray) 65%);
			}

			.color-mix-c-brightgray-d-mildwhite-35 {
				fill: color-mix(in srgb, var(--color-c-brightgray), var(--color-d-mildwhite) 35%);
			}

			.color-mix-c-brightgray-d-mildwhite-55 {
				fill: color-mix(in srgb, var(--color-c-brightgray), var(--color-d-mildwhite) 55%);
			}

			.color-mix-color-e-nearwhite-50 {
				fill: color-mix(in srgb, var(--color-data-color), var(--color-e-nearwhite) 50%);
			}

			.color-mix-d-mildwhite-e-nearwhite-5 {
				fill: color-mix(in srgb, var(--color-d-mildwhite), var(--color-e-nearwhite) 5%);
			}

			.color-mix-general-general-dim-60 {
				fill: color-mix(in srgb, var(--color-data-general), var(--color-data-general-dim) 60%);
			}

			.color-mix-gradient-7-middlegray-75 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-7-middlegray) 75%);
			}

			.color-mix-gradient-dim-0-black-40 {
				fill: color-mix(in srgb, var(--color-data-gradient-dim), var(--color-0-black) 40%);
			}

			.color-mix-gradient-dim-2-mildblack-35 {
				fill: color-mix(in srgb, var(--color-data-gradient-dim), var(--color-2-mildblack) 35%);
			}

			.color-mix-gradient-dim-4-dimgray-20 {
				fill: color-mix(in srgb, var(--color-data-gradient-dim), var(--color-4-dimgray) 20%);
			}

			.color-mix-gradient-dim-8-uppergray-35 {
				fill: color-mix(in srgb, var(--color-data-gradient-dim), var(--color-8-uppergray) 35%);
			}

			.color-mix-gradient-e-nearwhite-10 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 10%);
			}

			.color-mix-gradient-e-nearwhite-15 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 15%);
			}

			.color-mix-gradient-e-nearwhite-20 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 20%);
			}

			.color-mix-gradient-e-nearwhite-25 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 25%);
			}

			.color-mix-gradient-e-nearwhite-30 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 30%);
			}

			.color-mix-gradient-e-nearwhite-35 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 35%);
			}

			.color-mix-gradient-e-nearwhite-40 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 40%);
			}

			.color-mix-gradient-e-nearwhite-45 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 45%);
			}

			.color-mix-gradient-e-nearwhite-5 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 5%);
			}

			.color-mix-gradient-e-nearwhite-50 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 50%);
			}

			.color-mix-gradient-e-nearwhite-55 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 55%);
			}

			.color-mix-gradient-e-nearwhite-60 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 60%);
			}

			.color-mix-gradient-e-nearwhite-65 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 65%);
			}

			.color-mix-gradient-e-nearwhite-70 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 70%);
			}

			.color-mix-gradient-e-nearwhite-75 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 75%);
			}

			.color-mix-gradient-e-nearwhite-80 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 80%);
			}

			.color-mix-gradient-e-nearwhite-85 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 85%);
			}

			.color-mix-gradient-e-nearwhite-90 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 90%);
			}

			.color-mix-gradient-e-nearwhite-95 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-e-nearwhite) 95%);
			}

			.color-mix-gradient-gradient-dim-10 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 10%);
			}

			.color-mix-gradient-gradient-dim-15 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 15%);
			}

			.color-mix-gradient-gradient-dim-20 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 20%);
			}

			.color-mix-gradient-gradient-dim-25 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 25%);
			}

			.color-mix-gradient-gradient-dim-30 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 30%);
			}

			.color-mix-gradient-gradient-dim-35 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 35%);
			}

			.color-mix-gradient-gradient-dim-40 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 40%);
			}

			.color-mix-gradient-gradient-dim-45 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 45%);
			}

			.color-mix-gradient-gradient-dim-5 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 5%);
			}

			.color-mix-gradient-gradient-dim-50 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 50%);
			}

			.color-mix-gradient-gradient-dim-55 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 55%);
			}

			.color-mix-gradient-gradient-dim-60 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 60%);
			}

			.color-mix-gradient-gradient-dim-65 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 65%);
			}

			.color-mix-gradient-gradient-dim-70 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 70%);
			}

			.color-mix-gradient-gradient-dim-75 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 75%);
			}

			.color-mix-gradient-gradient-dim-80 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 80%);
			}

			.color-mix-gradient-gradient-dim-85 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 85%);
			}

			.color-mix-gradient-gradient-dim-90 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 90%);
			}

			.color-mix-gradient-gradient-dim-95 {
				fill: color-mix(in srgb, var(--color-data-gradient), var(--color-data-gradient-dim) 95%);
			}

			.color-mix-graphic-dim-0-black-30 {
				fill: color-mix(in srgb, var(--color-data-graphic-dim), var(--color-0-black) 30%);
			}

			.color-mix-raster-dim-7-middlegray-50 {
				fill: color-mix(in srgb, var(--color-data-raster-dim), var(--color-7-middlegray) 50%);
			}

			.color-mix-raster-dim-8-uppergray-40 {
				fill: color-mix(in srgb, var(--color-data-raster-dim), var(--color-8-uppergray) 40%);
			}

			.color-mix-raster-dim-8-uppergray-50 {
				fill: color-mix(in srgb, var(--color-data-raster-dim), var(--color-8-uppergray) 50%);
			}

			.color-mix-raster-e-nearwhite-15 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-e-nearwhite) 15%);
			}

			.color-mix-raster-e-nearwhite-35 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-e-nearwhite) 35%);
			}

			.color-mix-raster-e-nearwhite-40 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-e-nearwhite) 40%);
			}

			.color-mix-raster-e-nearwhite-50 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-e-nearwhite) 50%);
			}

			.color-mix-raster-e-nearwhite-65 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-e-nearwhite) 65%);
			}

			.color-mix-raster-e-nearwhite-85 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-e-nearwhite) 85%);
			}

			.color-mix-raster-raster-dim-30 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-data-raster-dim) 30%);
			}

			.color-mix-raster-raster-dim-35 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-data-raster-dim) 35%);
			}

			.color-mix-raster-raster-dim-50 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-data-raster-dim) 50%);
			}

			.color-mix-raster-raster-dim-65 {
				fill: color-mix(in srgb, var(--color-data-raster), var(--color-data-raster-dim) 65%);
			}

			.color-mix-vector-dim-0-black-30 {
				fill: color-mix(in srgb, var(--color-data-vector-dim), var(--color-0-black) 30%);
			}

			.color-mix-vector-vector-dim-50 {
				fill: color-mix(in srgb, var(--color-data-vector), var(--color-data-vector-dim) 50%);
			}
		}
	}
</style>
