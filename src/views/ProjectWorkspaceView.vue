<script setup lang="ts">
import { Folder, Plus } from "@lucide/vue";
import { computed, onMounted, reactive, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";

import { DesktopBridgeError } from "../lib/desktop";
import { formatDesktopError } from "../lib/errorMessage";
import { formatDateTime } from "../lib/format";
import {
  buildProjectWorkspacePath,
  findProjectByReference,
  getProjectWorkspaceSection,
  readRouteString,
  sanitizeProjectSwitchQuery,
  sanitizeSectionQuery,
} from "../lib/projectWorkspace";
import { projectWorkspaceSectionOptions, type ProjectWorkspaceSection } from "../lib/options";
import { useProjectsStore } from "../stores/projects";
import { useShellStore } from "../stores/shell";

const route = useRoute();
const router = useRouter();
const shell = useShellStore();
const projectsStore = useProjectsStore();
const { t } = useI18n({ useScope: "global" });

const createProjectForm = reactive({
  description: "",
  name: "",
  slug: "",
});

const projectSlug = computed(() => readRouteString(route.params.projectSlug));
const legacyProjectQuery = computed(() => readRouteString(route.query.project));
const selectedProject = computed(() => findProjectByReference(projectsStore.projects, projectSlug.value));
const currentSection = computed<ProjectWorkspaceSection>(() => getProjectWorkspaceSection(route));
const selectedProjectSlug = computed(() => selectedProject.value?.slug ?? projectSlug.value);
const showDetachedProject = computed(
  () => Boolean(projectSlug.value) && !selectedProject.value && currentSection.value === "approvals",
);
const navItems = computed(() =>
  projectWorkspaceSectionOptions.map((section) => ({
    key: section,
    label: t(`routes.projects.sections.${section}`),
  })),
);
const workspaceTitle = computed(
  () =>
    selectedProject.value?.name ??
    projectSlug.value ??
    t("projects.workspaceEmptyTitle"),
);
const workspaceSummary = computed(() => {
  if (selectedProject.value?.description) {
    return selectedProject.value.description;
  }
  if (selectedProject.value) {
    return t("projects.emptyDescription");
  }
  if (projectSlug.value) {
    return t("projects.pendingProjectSummary", { project: projectSlug.value });
  }
  return t("projects.workspaceEmptySummary");
});

function normalizeProjectSlug(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-_ ]/g, "-")
    .replace(/[\s_]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
}

watch(
  () => projectsStore.projects.map((item) => item.project_id).join("|"),
  async () => {
    if (projectsStore.loadingProjects) {
      return;
    }

    const fallbackProject = projectsStore.projects[0] ?? null;
    const legacyProject = findProjectByReference(projectsStore.projects, legacyProjectQuery.value);

    if (route.path === "/projects" && legacyProject) {
      await router.replace({
        path: buildProjectWorkspacePath(legacyProject.slug, "overview"),
        query: sanitizeSectionQuery("overview", route.query),
      });
      return;
    }

    if (route.path === "/projects" && fallbackProject) {
      await router.replace({
        path: buildProjectWorkspacePath(fallbackProject.slug, "overview"),
      });
      return;
    }

    if (projectSlug.value && !selectedProject.value && !showDetachedProject.value && fallbackProject) {
      await router.replace({
        path: buildProjectWorkspacePath(fallbackProject.slug, currentSection.value),
        query: sanitizeProjectSwitchQuery(currentSection.value, route.query),
      });
    }
  },
  { immediate: true },
);

onMounted(async () => {
  if (projectsStore.projects.length === 0) {
    await projectsStore.loadProjects();
  }
});

function sectionLocation(section: ProjectWorkspaceSection) {
  const targetSlug = selectedProjectSlug.value ?? projectsStore.projects[0]?.slug;
  if (!targetSlug) {
    return "/projects";
  }

  return {
    path: buildProjectWorkspacePath(targetSlug, section),
    query: sanitizeSectionQuery(section, route.query),
  };
}

async function selectProject(project: string) {
  await router.push({
    path: buildProjectWorkspacePath(project, currentSection.value),
    query: sanitizeProjectSwitchQuery(currentSection.value, route.query),
  });
}

async function submitCreateProject() {
  try {
    const created = await projectsStore.createProject({
      description: createProjectForm.description || null,
      name: createProjectForm.name,
      slug: createProjectForm.slug,
    });
    shell.pushNotice("success", t("notices.projectCreated"));
    createProjectForm.description = "";
    createProjectForm.name = "";
    createProjectForm.slug = "";
    await router.push({
      path: buildProjectWorkspacePath(created.slug, "overview"),
    });
  } catch (error) {
    if (await jumpToQueuedApproval(error, createProjectForm.slug)) {
      return;
    }
    shell.pushNotice("error", formatDesktopError(error, t));
  }
}

async function jumpToQueuedApproval(error: unknown, slugHint?: string) {
  if (
    error instanceof DesktopBridgeError &&
    error.code === "requires_human_review" &&
    typeof error.details === "object" &&
    error.details &&
    "approval_request_id" in error.details
  ) {
    const requestId = (error.details as Record<string, unknown>).approval_request_id;
    const targetSlug = (slugHint ? normalizeProjectSlug(slugHint) : undefined) || selectedProjectSlug.value;

    shell.pushNotice("info", t("notices.requestQueued"));

    if (targetSlug) {
      await router.push({
        path: buildProjectWorkspacePath(targetSlug, "approvals"),
        query: {
          approvalState: "pending",
          request: requestId as string,
        },
      });
      return true;
    }

    await router.push({
      path: "/approvals",
      query: {
        approvalState: "pending",
        request: requestId as string,
      },
    });
    return true;
  }

  return false;
}
</script>

<template>
  <section class="workspace-page">
    <aside class="workspace-registry">
      <div class="workspace-pane-header">
        <div>
          <p class="section-label">{{ t("projects.listKicker") }}</p>
          <h1 class="text-lg font-semibold text-[var(--text-main)]">{{ t("projects.listTitle") }}</h1>
        </div>
        <span class="status-pill">{{ projectsStore.projects.length }}</span>
      </div>

      <section class="panel-section">
        <p class="section-label">{{ t("projects.createProject") }}</p>
        <div class="space-y-3">
          <label class="form-field">
            <span class="field-label">{{ t("projects.fields.slug") }}</span>
            <input
              v-model="createProjectForm.slug"
              class="control-input"
              :placeholder="t('projects.placeholders.slug')"
            />
          </label>
          <label class="form-field">
            <span class="field-label">{{ t("projects.fields.name") }}</span>
            <input
              v-model="createProjectForm.name"
              class="control-input"
              :placeholder="t('projects.placeholders.projectName')"
            />
          </label>
          <label class="form-field">
            <span class="field-label">{{ t("projects.fields.description") }}</span>
            <textarea
              v-model="createProjectForm.description"
              class="control-textarea"
              :placeholder="t('projects.placeholders.projectDescription')"
            />
          </label>
          <button class="primary-action spotlight-surface" @click="submitCreateProject">
            <Plus :size="15" />
            {{ t("projects.createProjectAction") }}
          </button>
        </div>
      </section>

      <div class="workspace-pane-scroll">
        <div v-if="projectsStore.projects.length === 0" class="empty-state">
          {{ t("projects.workspaceEmptySummary") }}
        </div>
        <button
          v-for="project in projectsStore.projects"
          :key="project.project_id"
          v-spotlight
          class="list-row spotlight-surface"
          :class="{ 'list-row-active': selectedProject?.project_id === project.project_id }"
          @click="selectProject(project.slug)"
        >
          <Folder :size="16" />
          <div class="min-w-0 flex-1">
            <p class="truncate text-sm font-medium text-[var(--text-main)]">{{ project.name }}</p>
            <p class="truncate text-xs text-[var(--text-muted)]">{{ project.slug }}</p>
          </div>
          <span class="status-pill">{{ t(`status.project.${project.status}`) }}</span>
        </button>
      </div>
    </aside>

    <div class="workspace-shell">
      <header class="workspace-header">
        <div class="workspace-header-copy">
          <p class="section-label">{{ t("projects.workspaceKicker") }}</p>
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <h2 class="workspace-title">{{ workspaceTitle }}</h2>
              <p class="workspace-summary">{{ workspaceSummary }}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <span v-if="selectedProject" class="status-pill">{{ t(`status.project.${selectedProject.status}`) }}</span>
              <span v-else-if="projectSlug" class="status-pill status-pill-warning">
                {{ t("projects.pendingProject") }}
              </span>
              <span v-if="selectedProjectSlug" class="status-pill">{{ selectedProjectSlug }}</span>
              <span v-if="selectedProject" class="status-pill">
                {{ t("projects.updated") }} {{ formatDateTime(selectedProject.updated_at) }}
              </span>
            </div>
          </div>
        </div>

        <nav class="workspace-secondary-nav" :aria-label="t('projects.workspaceNavigation')">
          <RouterLink
            v-for="item in navItems"
            :key="item.key"
            v-slot="{ href, navigate }"
            :to="sectionLocation(item.key)"
            custom
          >
            <a
              :href="href"
              class="section-route-chip"
              :class="{ 'section-route-chip-active': currentSection === item.key }"
              :aria-current="currentSection === item.key ? 'page' : undefined"
              @click="navigate"
            >
              {{ item.label }}
            </a>
          </RouterLink>
        </nav>
      </header>

      <div class="workspace-content">
        <RouterView v-if="route.meta.workspaceSection" />
        <div v-else class="workspace-empty-state">
          <div class="workspace-empty-copy">
            <p class="section-label">{{ t("projects.workspaceKicker") }}</p>
            <h3 class="text-xl font-semibold text-[var(--text-main)]">
              {{ t("projects.workspaceLoadingTitle") }}
            </h3>
            <p class="mt-2 text-sm leading-6 text-[var(--text-muted)]">
              {{ t("projects.workspaceLoadingSummary") }}
            </p>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
