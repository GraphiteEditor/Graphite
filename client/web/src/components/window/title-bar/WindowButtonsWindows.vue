<template>
	<div class="windows window-button minimize" title="Minimize">
		<Icon :icon="'WindowButtonWinMinimize'" />
	</div>
	<div class="windows window-button maximize" title="Maximize" v-if="!maximized">
		<Icon :icon="'WindowButtonWinMaximize'" />
	</div>
	<div class="windows window-button restore-down" title="Restore Down" v-if="maximized">
		<Icon :icon="'WindowButtonWinRestoreDown'" />
	</div>
	<div class="windows window-button close" title="Close">
		<Icon :icon="'WindowButtonWinClose'" />
	</div>
</template>

<style lang="scss">
.windows.window-button {
	display: flex;
	align-items: center;
	padding: 0 17px;

	svg {
		fill: var(--color-e-nearwhite);
	}

	&:hover {
		background: var(--color-6-lowergray);

		svg {
			fill: var(--color-f-white);
		}
	}

	&.close:hover {
		background: #e81123;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import Icon from "../../widgets/labels/Icon.vue";

export default defineComponent({
	components: { Icon },
	props: {
		maximized: { type: Boolean, default: false },
	},
});
</script>
