<script lang="ts">
	import { IMAGE_BASE64_STRINGS } from "@graphite/utility-functions/images";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let image: string;
	export let width: string | undefined;
	export let height: string | undefined;
	export let tooltip: string | undefined = undefined;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<img src={IMAGE_BASE64_STRINGS[image]} style:width style:height class={`image-label ${className} ${extraClasses}`.trim()} title={tooltip} alt="" />

<style lang="scss" global>
	.image-label {
		width: auto;
		height: auto;

		+ .image-label.image-label {
			margin-left: 8px;
		}
	}
</style>
