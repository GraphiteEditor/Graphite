<template>
	<div class="separator" :class="[direction.toLowerCase(), type.toLowerCase()]">
		<div v-if="[SeparatorType.Section, SeparatorType.List].includes(type)"></div>
	</div>
</template>

<style lang="scss">
.separator {
	&.vertical {
		&.related {
			margin-top: 4px;
		}

		&.unrelated {
			margin-top: 8px;
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
		&.related {
			margin-left: 4px;
		}

		&.unrelated {
			margin-left: 8px;
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

<script lang="ts">
import { defineComponent } from "vue";

export enum SeparatorDirection {
	"Horizontal" = "Horizontal",
	"Vertical" = "Vertical",
}

export enum SeparatorType {
	"Related" = "Related",
	"Unrelated" = "Unrelated",
	"Section" = "Section",
	"List" = "List",
}

export default defineComponent({
	components: {},
	props: {
		direction: { type: String, default: SeparatorDirection.Horizontal },
		type: { type: String, default: SeparatorType.Unrelated },
	},
	data() {
		return {
			SeparatorDirection,
			SeparatorType,
		};
	},
});
</script>
