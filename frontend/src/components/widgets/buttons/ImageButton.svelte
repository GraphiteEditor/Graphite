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

<!-- ✅ Wrap image inside a button for accessibility -->
<button
	type="button"
	class={`image-button ${className} ${extraClasses}`.trim()}
	on:click={action}
	title={tooltip}
>
	<img
		src={IMAGE_BASE64_STRINGS[image]}
		{width}
		{height}
		alt={tooltip || "icon button"}
	/>
</button>

<style lang="scss" global>
	.image-button {
		background: none;
		border: none;
		padding: 0;
		cursor: pointer;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: 2px;

		+ .image-button {
			margin-left: 8px;
		}

		img {
			display: block;
			border-radius: 2px;
		}
	}
</style>
