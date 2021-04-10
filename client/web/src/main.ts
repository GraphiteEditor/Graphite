import { createApp } from "vue";
import App from "./App.vue";
import { attachResponseHandlerToPage } from "./response-handler";

attachResponseHandlerToPage();

createApp(App).mount("#app");
