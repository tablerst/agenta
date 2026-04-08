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
      meta: {
        titleKey: "routes.projects.title",
        kickerKey: "routes.projects.kicker",
      },
    },
    {
      path: "/tasks",
      name: "tasks",
      component: TasksView,
      meta: {
        titleKey: "routes.tasks.title",
        kickerKey: "routes.tasks.kicker",
      },
    },
    {
      path: "/approvals",
      name: "approvals",
      component: ApprovalsView,
      meta: {
        titleKey: "routes.approvals.title",
        kickerKey: "routes.approvals.kicker",
      },
    },
    {
      path: "/runtime",
      name: "runtime",
      component: RuntimeView,
      meta: {
        titleKey: "routes.runtime.title",
        kickerKey: "routes.runtime.kicker",
      },
    },
  ],
});
