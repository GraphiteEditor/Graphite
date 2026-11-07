<script lang="ts">
	import type { HistoryThumbnail } from "/wrapper/pkg/graphite_wasm_wrapper";

	export let thumbnail: HistoryThumbnail;
	// The images the frontend received, by ID
	export let images: Record<string, string>;
	export let large = false;
</script>

<span class="history-shot" class:large>
	<span class="image" data-tooltip-label="The document once this step was done">
		{#if images[thumbnail.whole]}<img src={images[thumbnail.whole]} alt="" />{/if}
	</span>
	{#if thumbnail.detail}
		<span class="image" data-tooltip-label="What this step changed, close up" data-tooltip-description="Green was added, red removed, and yellow changed.">
			{#if images[thumbnail.detail]}<img src={images[thumbnail.detail]} alt="" />{/if}
		</span>
	{/if}
</span>

<style lang="scss">
	.history-shot {
		flex: 0 0 auto;
		display: flex;
		gap: 2px;

		.image {
			width: 36px;
			height: 24px;
			border-radius: 2px;
			overflow: hidden;
			background-image: var(--color-transparent-checkered-background);
			background-size: var(--color-transparent-checkered-background-size-mini);
			background-position: var(--color-transparent-checkered-background-position-mini);
			background-repeat: var(--color-transparent-checkered-background-repeat);
		}

		&.large {
			gap: 4px;

			.image {
				// The size captures are rendered at, like layer thumbnails
				width: 72px;
				height: 48px;
				background-size: var(--color-transparent-checkered-background-size);
				background-position: var(--color-transparent-checkered-background-position);
			}
		}

		img {
			display: block;
			width: 100%;
			height: 100%;
		}
	}
</style>
