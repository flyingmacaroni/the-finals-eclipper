import { createApp } from "vue";
import App from "./App.vue";
import PrimeVue from 'primevue/config';
import {createRouter, createWebHistory} from 'vue-router';

import 'primevue/resources/themes/lara-dark-green/theme.css';
import "./styles.css";
import 'primeicons/primeicons.css'
import {routes} from "./router";
import Ripple from "primevue/ripple";

const router = createRouter({
    // 4. Provide the history implementation to use. We are using the hash history for simplicity here.
    history: createWebHistory(),
    routes, // short for `routes: routes`
});

const app = createApp(App);
app.use(router);
app.use(PrimeVue, {ripple: true});
app.directive('ripple', Ripple);
app.mount("#app");
