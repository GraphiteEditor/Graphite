<template>
	<LayoutCol class="main-window">
		<LayoutRow :class="'title-bar'">
			<TitleBar :platform="platform" :maximized="maximized" />
		</LayoutRow>
		<LayoutRow :class="'panel-container'">
			<PanelArea />
		</LayoutRow>
		<LayoutRow :class="'footer-bar'">
			<FooterBar />
		</LayoutRow>
	</LayoutCol>
</template>

<style lang="scss">
.main-window {
	height: 100%;
}

.title-bar {
	height: 28px;
	flex: 0 0 auto;
}

.panel-container {
	flex: 1 1 100%;
	height: 100%;
}

.footer-bar {
	height: 24px;
	flex: 0 0 auto;
}
</style>

<script lang="ts">
import { defineComponent } from "vue";
import LayoutRow from "../layout/LayoutRow.vue";
import LayoutCol from "../layout/LayoutCol.vue";
import PanelArea from "../panel-system/PanelArea.vue";
import TitleBar from "./title-bar/TitleBar.vue";
import FooterBar from "./footer-bar/FooterBar.vue";

export enum ApplicationPlatform {
	"Windows" = "Windows",
	"Mac" = "Mac",
	"Linux" = "Linux",
	"Web" = "Web",
}

export default defineComponent({
	components: {
		LayoutRow,
		LayoutCol,
		TitleBar,
		PanelArea,
		FooterBar,
	},
	data() {
		return {
			platform: ApplicationPlatform.Windows,
			maximized: true,
		};
	},
});
</script>
