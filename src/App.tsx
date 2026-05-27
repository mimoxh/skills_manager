import { ChangeEvent, useRef, useState } from "react";
import { TooltipProvider } from "./components/ui/tooltip";
import { Titlebar } from "./components/layout/Titlebar";
import { Sidebar } from "./components/layout/Sidebar";
import { CommandBar } from "./components/layout/CommandBar";
import { OverviewView } from "./components/views/OverviewView";
import { SkillsView } from "./components/views/SkillsView";
import { AgentsView } from "./components/views/AgentsView";
import { SettingsView } from "./components/views/SettingsView";
import { useAppState } from "./hooks/useAppState";
import type { ImportSkillFile } from "./types";

export type View = "overview" | "skills" | "agents" | "settings";

export default function App() {
  const folderInputRef = useRef<HTMLInputElement>(null);
  const archiveInputRef = useRef<HTMLInputElement>(null);
  const [view, setView] = useState<View>("skills");
  const [dragging, setDragging] = useState(false);

  const state = useAppState();

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

  function renderView() {
    switch (view) {
      case "skills":
        return (
          <SkillsView
            skills={state.filteredSkills}
            agents={state.agents}
            busy={state.busy}
            onDrop={state.handleSkillDrop}
            onDrag={setDragging}
            dragging={dragging}
            onFolder={() => folderInputRef.current?.click()}
            onArchive={() => archiveInputRef.current?.click()}
            onSync={state.syncSkillToAgents}
          />
        );
      case "agents":
        return (
          <AgentsView
            agents={state.filteredAgents}
            skills={state.skills}
            customAgent={state.customAgent}
            busy={state.busy}
            onCustomChange={state.setCustomAgent}
            onSaveCustom={state.saveCustomAgent}
          />
        );
      case "settings":
        return (
          <SettingsView
            repository={state.repository}
            busy={state.busy}
            onRepository={state.setRepository}
            onSave={state.saveRepository}
            discoveryPaths={state.discoveryPaths}
            onRefresh={state.refreshAll}
          />
        );
      default:
        return (
          <OverviewView
            skills={state.skills}
            agents={state.agents}
            onNavigate={setView}
            onFolder={() => folderInputRef.current?.click()}
            onArchive={() => archiveInputRef.current?.click()}
          />
        );
    }
  }

  return (
    <TooltipProvider>
      <div className="app-surface flex h-full w-full flex-col overflow-hidden">
        <Titlebar />
        <div className="flex min-h-0 flex-1">
          <Sidebar view={view} repository={state.repository} onNavigate={setView} />
          <main className="flex min-w-0 flex-1 flex-col">
            <div className="shrink-0 border-b border-[var(--color-border)] bg-[rgb(253_253_252_/_0.86)] px-6 py-4">
              <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-3">
                <CommandBar
                  query={state.query}
                  onQueryChange={state.setQuery}
                  busy={state.busy}
                  onRefresh={state.refreshAll}
                />
                <div className="flex min-h-8 items-center gap-2 rounded-md bg-[var(--color-surface)] px-3 text-xs text-[var(--color-text-secondary)]">
                  <span
                    className={`inline-block h-2 w-2 shrink-0 rounded-full ${
                      state.busy ? "bg-[var(--color-warning)]" : "bg-[var(--color-success)]"
                    }`}
                  />
                  <span className="truncate">{state.busy ? "正在处理..." : state.message}</span>
                </div>
              </div>
            </div>
            <div className="min-h-0 flex-1 overflow-auto px-6 py-6">
              <div className="mx-auto h-full w-full max-w-[1280px]">{renderView()}</div>
            </div>
          </main>
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
      </div>
    </TooltipProvider>
  );
}
