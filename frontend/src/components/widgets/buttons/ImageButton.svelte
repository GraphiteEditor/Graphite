<script lang="ts">
	import { IMAGE_BASE64_STRINGS } from "@graphite/utility-functions/images";

	type Props = {
		class?: string;
		classes?: Record<string, boolean>;
		image: string;
		width: string | undefined;
		height: string | undefined;
		tooltip?: string | undefined;
		// Callbacks
		action: (e?: MouseEvent) => void;
	};

	let { class: className = "", classes = {}, image, width, height, tooltip = undefined, action }: Props = $props();

	let extraClasses = $derived(
		Object.entries(classes)
			.flatMap(([className, stateName]) => (stateName ? [className] : []))
			.join(" "),
	);
</script>

<img src={IMAGE_BASE64_STRINGS[image]} style:width style:height class={`image-label ${className} ${extraClasses}`.trim()} title={tooltip} alt="" onclick={action} />

<style lang="scss" global>
	.image-label {
		width: auto;
		height: auto;

		+ .image-label.image-label {
			margin-left: 8px;
		}
	}
</style>
