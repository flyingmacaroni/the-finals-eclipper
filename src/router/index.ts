import ChooseFilePage from "../pages/ChooseFilePage.vue";
import {RouteRecordRaw} from "vue-router";
import {Route} from "../constants/routes.ts";
import ProcessingPage from "../pages/ProcessingPage.vue";
import EditClipsPage from "../pages/EditClipsPage.vue";

export const routes: RouteRecordRaw[] = [
    {path: Route.ChooseFile, component: ChooseFilePage, name: Route.ChooseFile},
    {path: Route.Processing, component: ProcessingPage, name: Route.Processing},
    {path: Route.EditClipsPage, component: EditClipsPage, name: Route.EditClipsPage},
]
