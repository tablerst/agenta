import { createRouter, createWebHistory } from "vue-router";

import ApprovalsView from "./views/ApprovalsView.vue";
import ProjectsView from "./views/ProjectsView.vue";
import RuntimeView from "./views/RuntimeView.vue";
import TasksView from "./views/TasksView.vue";

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      redirect: "/tasks",
    },
    {
      path: "/projects",
      name: "projects",
      component: ProjectsView,
    },
    {
      path: "/tasks",
      name: "tasks",
      component: TasksView,
    },
    {
      path: "/approvals",
      name: "approvals",
      component: ApprovalsView,
    },
    {
      path: "/runtime",
      name: "runtime",
      component: RuntimeView,
    },
  ],
});
