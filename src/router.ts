import { createRouter, createWebHistory } from "vue-router";

import LegacyRouteResolverView from "./views/LegacyRouteResolverView.vue";
import ProjectWorkspaceApprovalsView from "./views/ProjectWorkspaceApprovalsView.vue";
import ProjectWorkspaceOverviewView from "./views/ProjectWorkspaceOverviewView.vue";
import ProjectWorkspaceTasksView from "./views/ProjectWorkspaceTasksView.vue";
import ProjectWorkspaceVersionsView from "./views/ProjectWorkspaceVersionsView.vue";
import RuntimeHostView from "./views/RuntimeHostView.vue";
import RuntimeSyncView from "./views/RuntimeSyncView.vue";
import ProjectWorkspaceView from "./views/ProjectWorkspaceView.vue";
import RuntimeView from "./views/RuntimeView.vue";

const projectRouteMeta = {
  kickerKey: "routes.projects.kicker",
  titleKey: "routes.projects.title",
};

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/",
      redirect: "/projects",
    },
    {
      path: "/projects",
      name: "projects",
      component: ProjectWorkspaceView,
      meta: projectRouteMeta,
    },
    {
      path: "/projects/:projectSlug",
      component: ProjectWorkspaceView,
      meta: projectRouteMeta,
      children: [
        {
          path: "",
          redirect: (to) => ({
            name: "project-overview",
            params: to.params,
            query: to.query,
          }),
        },
        {
          path: "overview",
          name: "project-overview",
          component: ProjectWorkspaceOverviewView,
          meta: {
            kickerKey: "routes.projects.title",
            titleKey: "routes.projects.sections.overview",
            workspaceSection: "overview",
          },
        },
        {
          path: "versions",
          name: "project-versions",
          component: ProjectWorkspaceVersionsView,
          meta: {
            kickerKey: "routes.projects.title",
            titleKey: "routes.projects.sections.versions",
            workspaceSection: "versions",
          },
        },
        {
          path: "tasks",
          name: "project-tasks",
          component: ProjectWorkspaceTasksView,
          meta: {
            kickerKey: "routes.projects.title",
            titleKey: "routes.projects.sections.tasks",
            workspaceSection: "tasks",
          },
        },
        {
          path: "approvals",
          name: "project-approvals",
          component: ProjectWorkspaceApprovalsView,
          meta: {
            kickerKey: "routes.projects.title",
            titleKey: "routes.projects.sections.approvals",
            workspaceSection: "approvals",
          },
        },
      ],
    },
    {
      path: "/tasks",
      name: "legacy-tasks",
      component: LegacyRouteResolverView,
      meta: {
        kickerKey: "routes.projects.title",
        titleKey: "routes.projects.sections.tasks",
        legacySection: "tasks",
      },
    },
    {
      path: "/approvals",
      name: "legacy-approvals",
      component: LegacyRouteResolverView,
      meta: {
        kickerKey: "routes.projects.title",
        titleKey: "routes.projects.sections.approvals",
        legacySection: "approvals",
      },
    },
    {
      path: "/runtime",
      component: RuntimeView,
      meta: {
        titleKey: "routes.runtime.title",
        kickerKey: "routes.runtime.kicker",
      },
      children: [
        {
          path: "",
          redirect: {
            name: "runtime-host",
          },
        },
        {
          path: "host",
          name: "runtime-host",
          component: RuntimeHostView,
          meta: {
            titleKey: "routes.runtime.sections.host",
            kickerKey: "routes.runtime.title",
            runtimeSection: "host",
          },
        },
        {
          path: "sync",
          name: "runtime-sync",
          component: RuntimeSyncView,
          meta: {
            titleKey: "routes.runtime.sections.sync",
            kickerKey: "routes.runtime.title",
            runtimeSection: "sync",
          },
        },
      ],
    },
  ],
});
