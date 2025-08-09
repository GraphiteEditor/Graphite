<script lang="ts">
	let className = "";
	export { className as class };
	export let classes: Record<string, boolean> = {};

	export let url: string;
	export let width: string | undefined;
	export let height: string | undefined;
	export let tooltip: string | undefined = undefined;

	$: extraClasses = Object.entries(classes)
		.flatMap(([className, stateName]) => (stateName ? [className] : []))
		.join(" ");
</script>

<img src={url} style:width style:height class={`image-label ${className} ${extraClasses}`.trim()} title={tooltip} alt="" />

<style lang="scss" global>
	.image-label {
		width: auto;
		height: auto;
		border-radius: 2px;
		background-image: var(--color-transparent-checkered-background);
		background-size: var(--color-transparent-checkered-background-size);
		background-position: var(--color-transparent-checkered-background-position);
		background-repeat: var(--color-transparent-checkered-background-repeat);

		+ .image-label.image-label {
			margin-left: 8px;
		}
	}
</style>
