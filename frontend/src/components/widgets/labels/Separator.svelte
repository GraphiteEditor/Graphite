<script lang="ts">
	import { type SeparatorDirection, type SeparatorType } from "@graphite/wasm-communication/messages";

	export let direction: SeparatorDirection = "Horizontal";
	export let type: SeparatorType = "Unrelated";
</script>

<div class={`separator ${direction.toLowerCase()} ${type.toLowerCase()}`}>
	{#if type === "Section"}
		<div />
	{/if}
</div>

<style lang="scss" global>
	.separator {
		&.vertical {
			flex: 0 0 auto;

			&.related {
				height: 4px;
			}

			&.unrelated {
				height: 16px;
			}

			&.section {
				// If changing this, update `--separator-height` in `Document.svelte`
				margin: 12px 0;
				width: 100%;

				div {
					margin: 0 4px;
					height: 1px;
					width: calc(100% - 8px);
					background: var(--color-5-dullgray);
				}
			}
		}

		&.horizontal {
			flex: 0 0 auto;

			&.related {
				width: 4px;
			}

			&.unrelated {
				width: 16px;
			}

			&.section {
				margin: 0 12px;
				height: 100%;

				div {
					margin: 4px 0;
					height: calc(100% - 8px);
					width: 1px;
					background: var(--color-5-dullgray);
				}
			}
		}
	}
</style>
