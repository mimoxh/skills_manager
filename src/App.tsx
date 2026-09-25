import { ChangeEvent, Suspense, lazy, useEffect, useRef, useState } from "react";
import { TooltipProvider } from "./components/ui/tooltip";
import { Toast } from "./components/ui/Toast";
import { Titlebar } from "./components/layout/Titlebar";
import { Sidebar } from "./components/layout/Sidebar";
import { ImportAgentDialog } from "./components/views/ImportAgentDialog";
import { CatalogView } from "./components/views/CatalogView";
import { McpView } from "./components/views/McpView";
import { SettingsView } from "./components/views/SettingsView";
import { useAppState } from "./hooks/useAppState";
import { useTheme } from "./hooks/useTheme";
import { fileToUpload } from "./lib/utils";
import type { SkillsFilter } from "./types";

const SkillsView = lazy(() => import("./components/views/SkillsView").then((m) => ({ default: m.SkillsView })));
const OverviewView = lazy(() => import("./components/views/OverviewView").then((m) => ({ default: m.OverviewView })));
const AgentsView = lazy(() => import("./components/views/AgentsView").then((m) => ({ default: m.AgentsView })));

export type View = "overview" | "skills" | "catalog" | "agents" | "mcp" | "settings";

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
  const [focusSkillTitle, setFocusSkillTitle] = useState<string | null>(null);

  useEffect(() => {
    if ("__TAURI_INTERNALS__" in window) {
      import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
        getCurrentWindow().show();
      });
    }
  }, []);

  async function handleUploadChange(event: ChangeEvent<HTMLInputElement>) {
    const files = [...(event.target.files ?? [])];
    try {
      await state.importFiles(files[0]?.name ?? "upload", await Promise.all(files.map((f) => fileToUpload(f))));
    } catch (error) {
      state.showToast(String(error), "error");
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
              skills={state.skills}
              agents={state.agents}
              busy={state.busy}
              noFullCoverageTitles={state.noFullCoverageTitles}
              initialFilter={skillsFilter}
              focusSkillTitle={focusSkillTitle}
              onFocusedSkill={() => setFocusSkillTitle(null)}
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
              agents={state.agents}
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
            defaultSourceId={state.defaultCatalogSourceId}
            onQuery={state.setCatalogQuery}
            onSort={state.setCatalogSort}
            onFilters={state.setCatalogFilters}
            onSearch={state.searchCatalog}
            onEnsureCatalogLoaded={state.ensureCatalogLoaded}
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
      case "settings":
        return (
          <SettingsView
            palette={theme.palette}
            themeMode={theme.themeMode}
            resolvedTheme={theme.resolvedTheme}
            onPaletteChange={theme.setPalette}
            onThemeChange={theme.setThemeMode}
            syncConfig={state.syncConfig}
            syncStatus={state.syncStatus}
            syncConflicts={state.syncConflicts}
            syncBusy={state.syncBusy}
            onSaveSyncConfig={state.saveSyncConfig}
            onTestSyncConnection={state.testSyncConnection}
            onSyncNow={state.syncNow}
            onResolveSyncConflict={state.resolveSyncConflict}
            onSyncGc={state.runSyncGc}
            agents={state.agents}
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
        />
        <div className="main">
          <Titlebar />
          {state.pendingHubSkills.length > 0 && (
            <div style={{ padding: "8px 16px", background: "var(--accent-light)", display: "flex", alignItems: "center", gap: 12, fontSize: 12 }}>
              <span style={{ flex: 1 }}>{state.pendingHubSkills[0].message ?? `新 Skill「${state.pendingHubSkills[0].title}」已同步到中枢，尚未选择本机 Agent。`}{state.pendingHubSkills.length > 1 ? `（另有 ${state.pendingHubSkills.length - 1} 个待处理）` : ""}</span>
              {state.skills.some((skill) => skill.title === state.pendingHubSkills[0].title) && <button className="btn btn-primary btn-sm" type="button" onClick={() => { setView("skills"); setFocusSkillTitle(state.pendingHubSkills[0].title); void state.refreshAll(); }}>查看 Skill</button>}
            </div>
          )}
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

      <Toast message={state.toast} onDismiss={state.dismissToast} />
    </TooltipProvider>
  );
}
