import { ChangeEvent, Suspense, lazy, useEffect, useRef, useState } from "react";
import { TooltipProvider } from "./components/ui/tooltip";
import { Titlebar } from "./components/layout/Titlebar";
import { Sidebar } from "./components/layout/Sidebar";
import { ImportAgentDialog } from "./components/views/ImportAgentDialog";
import { CatalogView } from "./components/views/CatalogView";
import { McpView } from "./components/views/McpView";
import { useAppState } from "./hooks/useAppState";
import { useTheme } from "./hooks/useTheme";
import type { ImportSkillFile } from "./types";

const SkillsView = lazy(() => import("./components/views/SkillsView").then((m) => ({ default: m.SkillsView })));
const OverviewView = lazy(() => import("./components/views/OverviewView").then((m) => ({ default: m.OverviewView })));
const AgentsView = lazy(() => import("./components/views/AgentsView").then((m) => ({ default: m.AgentsView })));

export type View = "overview" | "skills" | "catalog" | "agents" | "mcp";
export type SkillsFilter = "all" | "covered" | "partial" | "needed";

function ViewLoading() {
  return (
    <div style={{ display: "flex", height: "100%", alignItems: "center", justifyContent: "center" }}>
      <div style={{ fontSize: 14, color: "var(--text-tertiary)" }}>加载中...</div>
    </div>
  );
}

export default function App() {
  const folderInputRef = useRef<HTMLInputElement>(null);
  const archiveInputRef = useRef<HTMLInputElement>(null);
  const [view, setView] = useState<View>("overview");
  const [skillsFilter, setSkillsFilter] = useState<SkillsFilter>("all");

  const state = useAppState();
  const theme = useTheme();

  useEffect(() => {
    if ("__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
        getCurrentWindow().show();
      });
    }
  }, []);

  async function fileToUpload(file: File, relativePath?: string): Promise<ImportSkillFile> {
    return {
      relativePath: relativePath || file.webkitRelativePath || file.name,
      bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
    };
  }

  async function handleUploadChange(event: ChangeEvent<HTMLInputElement>) {
    const files = [...(event.target.files ?? [])];
    try {
      await state.importFiles(files[0]?.name ?? "upload", await Promise.all(files.map((f) => fileToUpload(f))));
    } catch (error) {
      state.setMessage(String(error));
    } finally {
      if (folderInputRef.current) folderInputRef.current.value = "";
      if (archiveInputRef.current) archiveInputRef.current.value = "";
    }
  }

  function navigateTo(view: View, filter?: SkillsFilter) {
    setView(view);
    if (view === "skills" && filter) {
      setSkillsFilter(filter);
    }
  }

  function renderView() {
    switch (view) {
      case "skills":
        return (
          <Suspense fallback={<ViewLoading />}>
            <SkillsView
              skills={state.filteredSkills}
              agents={state.agents}
              busy={state.busy}
              noFullCoverageTitles={state.noFullCoverageTitles}
              initialFilter={skillsFilter}
              onDrop={state.handleSkillDrop}
              onFolder={() => folderInputRef.current?.click()}
              onArchive={() => archiveInputRef.current?.click()}
              onSync={state.syncSkillToAgents}
              onUninstall={state.uninstallSkillFromAgents}
              onLoadReadme={state.loadSkillReadme}
              onRefresh={state.refreshAll}
              onToggleNoFullCoverage={state.toggleNoFullCoverage}
              onSetSkillTags={state.setSkillTags}
            />
          </Suspense>
        );
      case "agents":
        return (
          <Suspense fallback={<ViewLoading />}>
            <AgentsView
              agents={state.filteredAgents}
              skills={state.skills}
              customAgent={state.customAgent}
              busy={state.busy}
              onCustomChange={state.setCustomAgent}
              onSaveCustom={state.saveCustomAgent}
              onSaveAgent={state.saveAgent}
              onSetAgentTags={state.setAgentTags}
              onDelete={state.deleteAgent}
              onSync={state.syncSkillToAgents}
              onUninstall={state.uninstallSkillFromAgents}
              onRepairCowork={state.repairClaudeCoworkManifest}
              onRefresh={state.refreshAll}
            />
          </Suspense>
        );
      case "catalog":
        return (
          <CatalogView
            busy={state.catalogBusy || state.busy}
            agents={state.agents}
            localSkills={state.skills}
            sources={state.catalogSources}
            skills={state.catalogSkills}
            total={state.catalogTotal}
            page={state.catalogPage}
            pageSize={state.catalogPageSize}
            hasMore={state.catalogHasMore}
            startupRefreshing={state.catalogStartupRefreshing}
            refreshStatuses={state.catalogRefreshStatuses}
            query={state.catalogQuery}
            sort={state.catalogSort}
            filters={state.catalogFilters}
            onQuery={state.setCatalogQuery}
            onSort={state.setCatalogSort}
            onFilters={state.setCatalogFilters}
            onSearch={state.searchCatalog}
            onPage={state.changeCatalogPage}
            onRefreshSource={state.refreshCatalogSource}
            onRefreshStatus={state.refreshCatalogStatus}
            onStartRefresh={state.startCatalogRefresh}
            onCancelRefresh={state.cancelCatalogRefresh}
            onSaveSource={state.saveCatalogSource}
            onInstallSkill={state.installCatalogSkill}
            onUninstallSkill={state.uninstallSkillFromAgents}
          />
        );
      case "mcp":
        return (
          <McpView
            servers={state.mcpServers}
            agents={state.agents}
            busy={state.busy}
            noFullCoverageMcpTitles={state.noFullCoverageMcpTitles}
            onAdd={state.addMcpServer}
            onUpdate={state.updateMcpServer}
            onRemove={state.removeMcpServer}
            onToggle={state.toggleMcpServer}
            onRefresh={state.refreshMcpServers}
            onSyncToAgents={state.syncMcpServerToAgents}
            onRemoveFromAgents={state.removeMcpServerFromAgents}
            onToggleNoFullCoverage={state.toggleMcpNoFullCoverage}
          />
        );
      default:
        return (
          <Suspense fallback={<ViewLoading />}>
            <OverviewView
              skills={state.skills}
              agents={state.agents}
              noFullCoverageTitles={state.noFullCoverageTitles}
              onNavigate={navigateTo}
              onFolder={() => folderInputRef.current?.click()}
              onArchive={() => archiveInputRef.current?.click()}
            />
          </Suspense>
        );
    }
  }

  return (
    <TooltipProvider>
      <div className="app">
        <Sidebar
          view={view}
          onNavigate={setView}
          skillCount={state.skills.length}
          agentCount={state.agents.length}
          themeMode={theme.themeMode}
          resolvedTheme={theme.resolvedTheme}
          onThemeChange={theme.setThemeMode}
        />
        <div className="main">
          <Titlebar />
          <div className="content">
            {state.isInitialLoading ? (
              <div style={{ display: "flex", height: "100%", alignItems: "center", justifyContent: "center" }}>
                <div style={{ fontSize: 14, color: "var(--text-tertiary)" }}>正在加载...</div>
              </div>
            ) : (
              renderView()
            )}
          </div>
        </div>
      </div>

      <input
        ref={folderInputRef}
        className="hidden-file-input"
        type="file"
        multiple
        // @ts-expect-error Chromium supports folder uploads through webkitdirectory.
        webkitdirectory=""
        onChange={handleUploadChange}
      />
      <input
        ref={archiveInputRef}
        className="hidden-file-input"
        type="file"
        accept=".zip"
        onChange={handleUploadChange}
      />

      {state.pendingImport && (
        <ImportAgentDialog
          agents={state.agents}
          busy={state.busy}
          fileName={state.pendingImport.fileName}
          onClose={state.cancelImport}
          onImport={state.executeImport}
        />
      )}
    </TooltipProvider>
  );
}
