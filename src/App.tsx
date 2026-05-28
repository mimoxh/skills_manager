import { ChangeEvent, Suspense, lazy, useEffect, useRef, useState } from "react";
import { TooltipProvider } from "./components/ui/tooltip";
import { Titlebar } from "./components/layout/Titlebar";
import { Sidebar } from "./components/layout/Sidebar";
import { CommandBar } from "./components/layout/CommandBar";
import { SkillsView } from "./components/views/SkillsView";
import { useAppState } from "./hooks/useAppState";
import type { ImportSkillFile } from "./types";

const OverviewView = lazy(() => import("./components/views/OverviewView").then((m) => ({ default: m.OverviewView })));
const AgentsView = lazy(() => import("./components/views/AgentsView").then((m) => ({ default: m.AgentsView })));

export type View = "overview" | "skills" | "agents";

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
  const [view, setView] = useState<View>("skills");

  const state = useAppState();

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
    await state.importFiles(files[0]?.name ?? "upload", await Promise.all(files.map((f) => fileToUpload(f))));
    if (folderInputRef.current) folderInputRef.current.value = "";
    if (archiveInputRef.current) archiveInputRef.current.value = "";
  }

  const showSearch = view === "skills" || view === "agents";

  function renderView() {
    switch (view) {
      case "skills":
        return (
          <SkillsView
            skills={state.filteredSkills}
            agents={state.agents}
            busy={state.busy}
            onDrop={state.handleSkillDrop}
            onFolder={() => folderInputRef.current?.click()}
            onArchive={() => archiveInputRef.current?.click()}
            onSync={state.syncSkillToAgents}
          />
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
              onDelete={state.deleteAgent}
              onSync={state.syncSkillToAgents}
            />
          </Suspense>
        );
      default:
        return (
          <Suspense fallback={<ViewLoading />}>
            <OverviewView
              skills={state.skills}
              agents={state.agents}
              onNavigate={setView}
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
        <Sidebar view={view} onNavigate={setView} skillCount={state.skills.length} agentCount={state.agents.length} />
        <div className="main">
          <Titlebar />
          {showSearch && (
            <CommandBar
              query={state.query}
              onQueryChange={state.setQuery}
              busy={state.busy}
              onRefresh={state.refreshAll}
              onFolder={() => folderInputRef.current?.click()}
              onArchive={() => archiveInputRef.current?.click()}
            />
          )}
          {showSearch && (
            <div className="statusbar">
              <span className={`status-dot${state.isInitialLoading || state.busy ? " busy" : ""}`} />
              <span>{state.isInitialLoading ? "正在加载..." : state.busy ? "正在处理..." : state.message}</span>
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
    </TooltipProvider>
  );
}
