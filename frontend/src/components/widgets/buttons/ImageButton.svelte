<script lang="ts">
	import { IMAGE_BASE64_STRINGS } from "@graphite/utility-functions/images";

	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let image: string;
	export let width: string | undefined;
	export let height: string | undefined;
	export let tooltip: string | undefined = undefined;
	// Callbacks
	export let action: (e?: MouseEvent) => void;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<img src={IMAGE_BASE64_STRINGS[image]} style:width style:height class={`image-button ${className} ${extraClasses}`.trim()} title={tooltip} alt="" on:click={action} />

<style lang="scss" global>
	.image-button {
		width: auto;
		height: auto;
		border-radius: 2px;

		+ .image-button.image-button {
			margin-left: 8px;
		}
	}
</style>
