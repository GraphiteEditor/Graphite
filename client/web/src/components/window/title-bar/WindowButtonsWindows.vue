<template>
	<div class="windows window-button minimize" title="Minimize">
		<WindowButtonWinMinimize width="12" height="12" />
	</div>
	<div class="windows window-button maximize" title="Maximize" v-if="!maximized">
		<WindowButtonWinMaximize width="12" height="12" />
	</div>
	<div class="windows window-button restore-down" title="Restore Down" v-if="maximized">
		<WindowButtonWinRestoreDown width="12" height="12" />
	</div>
	<div class="windows window-button close" title="Close">
		<WindowButtonWinClose width="12" height="12" />
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
import WindowButtonWinMinimize from "../../../../assets/12px-solid/window-button-win-minimize.svg";
import WindowButtonWinMaximize from "../../../../assets/12px-solid/window-button-win-maximize.svg";
import WindowButtonWinRestoreDown from "../../../../assets/12px-solid/window-button-win-restore-down.svg";
import WindowButtonWinClose from "../../../../assets/12px-solid/window-button-win-close.svg";

export default defineComponent({
	components: {
		WindowButtonWinMinimize,
		WindowButtonWinMaximize,
		WindowButtonWinRestoreDown,
		WindowButtonWinClose,
	},
	props: {
		maximized: { type: Boolean, default: false },
	},
});
</script>
