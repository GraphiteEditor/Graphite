<script lang="ts">
	import { type SeparatorDirection, type SeparatorType } from "~/src/wasm-communication/messages";

	export let direction: SeparatorDirection = "Horizontal";
	export let type: SeparatorType = "Unrelated";
</script>

<div class={`separator ${direction.toLowerCase()} ${type.toLowerCase()}`}>
	{#if ["Section", "List"].includes(type)}
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
				height: 8px;
			}

			&.section,
			&.list {
				width: 100%;

				div {
					height: 1px;
					width: calc(100% - 8px);
					margin: 0 4px;
					background: var(--color-7-middlegray);
				}
			}

			&.section {
				margin: 8px 0;
			}

			&.list {
				margin: 4px 0;
			}
		}

		&.horizontal {
			flex: 0 0 auto;

			&.related {
				width: 4px;
			}

			&.unrelated {
				width: 8px;
			}

			&.section,
			&.list {
				height: 100%;

				div {
					height: calc(100% - 8px);
					width: 1px;
					margin: 4px 0;
					background: var(--color-7-middlegray);
				}
			}

			&.section {
				margin: 0 8px;
			}

			&.list {
				margin: 0 4px;
			}
		}
	}
</style>
