import { createApp } from "vue";
import { createPinia } from "pinia";

import App from "./App.vue";
import { spotlightDirective } from "./directives/spotlight";
import { i18n } from "./i18n";
import "./main.css";
import { router } from "./router";

const app = createApp(App);

app.use(createPinia());
app.use(i18n);
app.use(router);
app.directive("spotlight", spotlightDirective);
app.mount("#app");
