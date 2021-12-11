<template>
	<div class="header-third">
		<WindowButtonsMac :maximized="maximized" v-if="platform === ApplicationPlatform.Mac" />
		<MenuBarInput v-if="platform !== ApplicationPlatform.Mac" />
	</div>
	<div class="header-third">
		<WindowTitle :title="`${documents.state.documents[documents.state.activeDocumentIndex].displayName} - Graphite`" />
	</div>
	<div class="header-third">
		<WindowButtonsWindows :maximized="maximized" v-if="platform === ApplicationPlatform.Windows || platform === ApplicationPlatform.Linux" />
		<WindowButtonsWeb :maximized="maximized" v-if="platform === ApplicationPlatform.Web" />
	</div>
</template>

<style lang="scss">
.header-third {
	display: flex;
	flex: 1 1 100%;

	&:nth-child(1) {
		justify-content: flex-start;
	}

	&:nth-child(2) {
		justify-content: center;
	}

	&:nth-child(3) {
		justify-content: flex-end;
	}
}
</style>

<script lang="ts">
import { defineComponent } from "vue";

import WindowTitle from "@/components/window/title-bar/WindowTitle.vue";
import WindowButtonsWindows from "@/components/window/title-bar/WindowButtonsWindows.vue";
import WindowButtonsMac from "@/components/window/title-bar/WindowButtonsMac.vue";
import WindowButtonsWeb from "@/components/window/title-bar/WindowButtonsWeb.vue";
import MenuBarInput from "@/components/widgets/inputs/MenuBarInput.vue";
import { ApplicationPlatform } from "@/components/window/MainWindow.vue";

export default defineComponent({
	inject: ["documents"],
	props: {
		platform: { type: String, required: true },
		maximized: { type: Boolean, required: true },
	},
	data() {
		return {
			ApplicationPlatform,
		};
	},
	components: {
		MenuBarInput,
		WindowTitle,
		WindowButtonsWindows,
		WindowButtonsMac,
		WindowButtonsWeb,
	},
});
</script>
