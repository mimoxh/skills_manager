import { ChangeEvent, DragEvent, MouseEvent, ReactNode, useEffect, useMemo, useRef, useState } from "react";
import {
  Archive,
  Bot,
  Check,
  ChevronRight,
  FolderArchive,
  FolderPlus,
  Gauge,
  Maximize2,
  Minus,
  PanelRight,
  RefreshCw,
  RotateCcw,
  Search,
  Settings,
  ShieldAlert,
  Sparkles,
  Trash2,
  UploadCloud,
  X,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "./api";
import type { AgentProfile, ConflictPolicy, ImportSkillFile, InstallResult, InstallState, SkillSummary } from "./types";

type View = "overview" | "skills" | "agents" | "sync" | "settings";

const emptyCustom: AgentProfile = {
  id: "",
  name: "",
  type: "custom",
  skillsPath: "",
  adapterConfig: {},
};

const views: Array<{ id: View; label: string; icon: ReactNode }> = [
  { id: "overview", label: "概览", icon: <Gauge size={17} /> },
  { id: "skills", label: "Skills", icon: <Sparkles size={17} /> },
  { id: "agents", label: "Agents", icon: <Bot size={17} /> },
  { id: "sync", label: "同步", icon: <RefreshCw size={17} /> },
  { id: "settings", label: "设置", icon: <Settings size={17} /> },
];

const policyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "prompt", label: "提示冲突", helper: "遇到冲突时先停下" },
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新" },
  { value: "skip", label: "跳过冲突", helper: "只安装安全项目" },
  { value: "rename", label: "另存副本", helper: "生成独立副本" },
];

function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function appWindow() {
  return hasTauriRuntime() ? getCurrentWindow() : null;
}

function agentLabel(agent: AgentProfile) {
  return `${agent.name} (${agent.type})`;
}

function shortPath(path: string) {
  if (!path) return "未设置";
  return path.length > 48 ? `...${path.slice(-45)}` : path;
}

function statusText(status?: string) {
  switch (status) {
    case "installed":
      return "已安装";
    case "stale":
      return "需更新";
    case "conflict":
      return "冲突";
    default:
      return "未安装";
  }
}

function actionText(action: InstallResult["action"]) {
  switch (action) {
    case "installed":
      return "已安装";
    case "updated":
      return "已更新";
    case "skipped":
      return "已跳过";
    case "renamed":
      return "已另存";
    default:
      return action;
  }
}

function WindowFrame({ children }: { children: ReactNode }) {
  const [maximized, setMaximized] = useState(false);

  async function refreshMaximized() {
    const win = appWindow();
    if (!win) return;
    setMaximized(await win.isMaximized());
  }

  useEffect(() => {
    void refreshMaximized();
  }, []);

  async function startDrag(event: MouseEvent<HTMLElement>) {
    if (event.button !== 0 || event.detail > 1) return;
    await appWindow()?.startDragging();
  }

  async function toggleMaximize() {
    await appWindow()?.toggleMaximize();
    await refreshMaximized();
  }

  return (
    <div className={`window-stage ${maximized ? "is-maximized" : ""}`}>
      <div className="window-frame">
        <header className="window-titlebar" onMouseDown={startDrag} onDoubleClick={toggleMaximize}>
          <div className="window-brand">
            <div className="app-mark">
              <Sparkles size={15} />
            </div>
            <span>Skills Manager</span>
          </div>
          <div className="drag-spacer" />
          <div className="window-actions" onMouseDown={(event) => event.stopPropagation()} onDoubleClick={(event) => event.stopPropagation()}>
            <button className="window-button" onClick={() => appWindow()?.minimize()} title="最小化" type="button">
              <Minus size={15} />
            </button>
            <button className="window-button" onClick={toggleMaximize} title={maximized ? "还原" : "最大化"} type="button">
              <Maximize2 size={14} />
            </button>
            <button className="window-button close" onClick={() => appWindow()?.close()} title="关闭" type="button">
              <X size={16} />
            </button>
          </div>
        </header>
        {children}
      </div>
    </div>
  );
}

export default function App() {
  const folderInputRef = useRef<HTMLInputElement>(null);
  const archiveInputRef = useRef<HTMLInputElement>(null);
  const [view, setView] = useState<View>("overview");
  const [repository, setRepository] = useState("");
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [agents, setAgents] = useState<AgentProfile[]>([]);
  const [states, setStates] = useState<InstallState[]>([]);
  const [selectedSkills, setSelectedSkills] = useState<string[]>([]);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [customAgent, setCustomAgent] = useState<AgentProfile>(emptyCustom);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("prompt");
  const [message, setMessage] = useState("正在加载本地 skills 和 agent 配置...");
  const [results, setResults] = useState<InstallResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [draggingSkills, setDraggingSkills] = useState(false);
  const [query, setQuery] = useState("");

  const stateByPair = useMemo(() => {
    const map = new Map<string, InstallState>();
    states.forEach((state) => map.set(`${state.agentId}:${state.skillId}`, state));
    return map;
  }, [states]);

  const filteredSkills = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return skills;
    return skills.filter((skill) => {
      const manifest = skill.manifest;
      return [manifest.name, manifest.id, manifest.description ?? "", ...(manifest.tags ?? [])].some((value) =>
        value.toLowerCase().includes(normalized),
      );
    });
  }, [query, skills]);

  const filteredAgents = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return agents;
    return agents.filter((agent) => [agent.name, agent.id, agent.type, agent.skillsPath].some((value) => value.toLowerCase().includes(normalized)));
  }, [agents, query]);

  const selectedSkill = selectedSkills[0] ? skills.find((skill) => skill.manifest.id === selectedSkills[0]) : undefined;
  const selectedAgent = selectedAgents[0] ? agents.find((agent) => agent.id === selectedAgents[0]) : undefined;
  const installedCount = states.filter((state) => state.status === "installed").length;
  const conflictCount = states.filter((state) => state.status === "conflict").length;

  async function refreshAll() {
    setBusy(true);
    try {
      const [repo, scanned, savedAgents, detected, installStates] = await Promise.all([
        api.getRepository(),
        api.scanSkills().catch(() => []),
        api.listAgents(),
        api.detectAgents(),
        api.listInstallState().catch(() => []),
      ]);
      setRepository(repo ?? "");
      setSkills(scanned);
      const merged = new Map<string, AgentProfile>();
      [...savedAgents, ...detected].forEach((agent) => merged.set(agent.id, agent));
      const nextAgents = [...merged.values()];
      setAgents(nextAgents);
      setStates(installStates);
      setSelectedSkills((prev) => prev.filter((id) => scanned.some((skill) => skill.manifest.id === id)));
      setSelectedAgents((prev) => prev.filter((id) => nextAgents.some((agent) => agent.id === id)));
      setMessage(`已加载 ${scanned.length} 个 skills，${nextAgents.length} 个 agent 配置。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refreshAll();
  }, []);

  async function saveRepository() {
    setBusy(true);
    try {
      await api.setRepository(repository.trim());
      setMessage("主仓库已保存，正在重新扫描...");
      await refreshAll();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function saveCustomAgent() {
    const agent = {
      ...customAgent,
      id: customAgent.id || crypto.randomUUID(),
      name: customAgent.name.trim(),
      skillsPath: customAgent.skillsPath.trim(),
    };
    if (!agent.name || !agent.skillsPath) {
      setMessage("自定义 Agent 需要填写名称和 Skills 安装目录。");
      return;
    }
    setBusy(true);
    try {
      await api.addAgent(agent);
      setCustomAgent(emptyCustom);
      await refreshAll();
      setSelectedAgents((prev) => [...new Set([...prev, agent.id])]);
      setMessage(`已添加 ${agent.name}，可以选择 skills 进行同步。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function fileToUpload(file: File, relativePath?: string): Promise<ImportSkillFile> {
    return {
      relativePath: relativePath || file.webkitRelativePath || file.name,
      bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
    };
  }

  async function collectEntryFiles(entry: FileSystemEntry, prefix = ""): Promise<ImportSkillFile[]> {
    if (entry.isFile) {
      const file = await new Promise<File>((resolve, reject) => {
        (entry as FileSystemFileEntry).file(resolve, reject);
      });
      return [await fileToUpload(file, `${prefix}${file.name}`)];
    }

    if (!entry.isDirectory) return [];

    const directory = entry as FileSystemDirectoryEntry;
    const reader = directory.createReader();
    const children = await new Promise<FileSystemEntry[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    const nested = await Promise.all(children.map((child) => collectEntryFiles(child, `${prefix}${directory.name}/`)));
    return nested.flat();
  }

  async function importFiles(fileName: string, files: ImportSkillFile[]) {
    if (!files.length) {
      setMessage("没有可导入的文件。");
      return;
    }

    setBusy(true);
    try {
      const result = await api.importSkillUpload(fileName, files);
      await refreshAll();
      setMessage(result.message);
      setView("skills");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
      setDraggingSkills(false);
      if (folderInputRef.current) folderInputRef.current.value = "";
      if (archiveInputRef.current) archiveInputRef.current.value = "";
    }
  }

  async function handleUploadChange(event: ChangeEvent<HTMLInputElement>) {
    const files = [...(event.target.files ?? [])];
    await importFiles(files[0]?.name ?? "upload", await Promise.all(files.map((file) => fileToUpload(file))));
  }

  async function handleSkillDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    const entries = [...event.dataTransfer.items]
      .map((item) => item.webkitGetAsEntry?.())
      .filter((entry): entry is FileSystemEntry => Boolean(entry));
    if (entries.length) {
      const files = (await Promise.all(entries.map((entry) => collectEntryFiles(entry)))).flat();
      await importFiles(entries[0].name, files);
      return;
    }

    const files = [...event.dataTransfer.files];
    await importFiles(files[0]?.name ?? "upload", await Promise.all(files.map((file) => fileToUpload(file))));
  }

  async function installSelected() {
    if (!selectedSkills.length || !selectedAgents.length) {
      setMessage("请至少选择一个 skill 和一个 agent。");
      return;
    }
    if (conflictPolicy === "prompt") {
      const hasConflict = selectedSkills.some((skillId) =>
        selectedAgents.some((agentId) => stateByPair.get(`${agentId}:${skillId}`)?.status === "conflict"),
      );
      if (hasConflict) {
        setMessage("检测到冲突。请先选择备份覆盖、跳过冲突或另存副本策略。");
        return;
      }
    }
    setBusy(true);
    try {
      const installResults = await api.installSkills(selectedSkills, selectedAgents, conflictPolicy);
      setResults(installResults);
      await refreshAll();
      setMessage(`完成 ${installResults.length} 个同步任务。`);
      setView("sync");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  function toggleSkill(skillId: string) {
    setSelectedSkills((prev) => (prev.includes(skillId) ? prev.filter((id) => id !== skillId) : [...prev, skillId]));
  }

  function toggleAgent(agentId: string) {
    setSelectedAgents((prev) => (prev.includes(agentId) ? prev.filter((id) => id !== agentId) : [...prev, agentId]));
  }

  function primaryView() {
    switch (view) {
      case "skills":
        return <SkillsView skills={filteredSkills} selectedSkills={selectedSkills} dragging={draggingSkills} busy={busy} onToggle={toggleSkill} onDrop={handleSkillDrop} onDrag={setDraggingSkills} onFolder={() => folderInputRef.current?.click()} onArchive={() => archiveInputRef.current?.click()} />;
      case "agents":
        return <AgentsView agents={filteredAgents} selectedAgents={selectedAgents} states={states} customAgent={customAgent} busy={busy} onToggle={toggleAgent} onCustomChange={setCustomAgent} onSaveCustom={saveCustomAgent} />;
      case "sync":
        return <SyncView selectedSkills={selectedSkills} selectedAgents={selectedAgents} agents={agents} stateByPair={stateByPair} conflictPolicy={conflictPolicy} results={results} busy={busy} onPolicy={setConflictPolicy} onInstall={installSelected} onRefresh={refreshAll} />;
      case "settings":
        return <SettingsView repository={repository} busy={busy} onRepository={setRepository} onSave={saveRepository} />;
      default:
        return <OverviewView skills={skills} agents={agents} states={states} results={results} onView={setView} onFolder={() => folderInputRef.current?.click()} onArchive={() => archiveInputRef.current?.click()} />;
    }
  }

  return (
    <WindowFrame>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="sidebar-head">
            <div className="sidebar-logo">
              <Sparkles size={18} />
            </div>
            <div>
              <strong>Skills Manager</strong>
              <span>桌面工作台</span>
            </div>
          </div>

          <nav className="nav-list">
            {views.map((item) => (
              <button key={item.id} className={`nav-item ${view === item.id ? "active" : ""}`} onClick={() => setView(item.id)} type="button">
                {item.icon}
                <span>{item.label}</span>
              </button>
            ))}
          </nav>

          <div className="sidebar-foot">
            <span>主仓库</span>
            <strong title={repository}>{shortPath(repository)}</strong>
          </div>
        </aside>

        <section className="workbench">
          <header className="command-bar">
            <div className="search-box">
              <Search size={17} />
              <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索 skill、agent 或路径" />
            </div>
            <button className="secondary-button" onClick={refreshAll} disabled={busy} type="button">
              <RefreshCw size={16} />
              刷新
            </button>
            <button className="primary-button" onClick={installSelected} disabled={busy || !selectedSkills.length || !selectedAgents.length} type="button">
              <RefreshCw size={16} />
              同步选中
            </button>
          </header>

          <section className="status-strip" aria-live="polite">
            <span className={busy ? "status-dot busy" : "status-dot"} />
            <span>{busy ? "正在处理..." : message}</span>
          </section>

          <main className="content-grid">
            <section className="content-main">{primaryView()}</section>
            <RightPanel
              skills={skills}
              agents={agents}
              selectedSkill={selectedSkill}
              selectedAgent={selectedAgent}
              selectedSkills={selectedSkills}
              selectedAgents={selectedAgents}
              installedCount={installedCount}
              conflictCount={conflictCount}
              conflictPolicy={conflictPolicy}
              onPolicy={setConflictPolicy}
              onInstall={installSelected}
              busy={busy}
            />
          </main>
        </section>

        <input
          ref={folderInputRef}
          className="hidden-file-input"
          type="file"
          multiple
          // @ts-expect-error Chromium supports folder uploads through webkitdirectory.
          webkitdirectory=""
          onChange={handleUploadChange}
        />
        <input ref={archiveInputRef} className="hidden-file-input" type="file" accept=".zip" onChange={handleUploadChange} />
      </div>
    </WindowFrame>
  );
}

function OverviewView({
  skills,
  agents,
  states,
  results,
  onView,
  onFolder,
  onArchive,
}: {
  skills: SkillSummary[];
  agents: AgentProfile[];
  states: InstallState[];
  results: InstallResult[];
  onView: (view: View) => void;
  onFolder: () => void;
  onArchive: () => void;
}) {
  const stale = states.filter((state) => state.status === "stale").length;
  const conflicts = states.filter((state) => state.status === "conflict").length;

  return (
    <div className="view-stack">
      <div className="metric-grid">
        <Metric label="Skills" value={skills.length} helper="主仓库中可识别项目" />
        <Metric label="Agents" value={agents.length} helper="可同步目标配置" />
        <Metric label="待处理" value={stale + conflicts} helper="需更新或冲突项目" />
      </div>

      <section className="panel-section">
        <SectionTitle title="快速操作" subtitle="导入、扫描并同步本地 skills" />
        <div className="quick-actions">
          <button className="action-tile" onClick={onFolder} type="button">
            <FolderPlus size={20} />
            <span>导入文件夹</span>
          </button>
          <button className="action-tile" onClick={onArchive} type="button">
            <FolderArchive size={20} />
            <span>导入 zip</span>
          </button>
          <button className="action-tile" onClick={() => onView("sync")} type="button">
            <RefreshCw size={20} />
            <span>打开同步</span>
          </button>
        </div>
      </section>

      <section className="panel-section">
        <SectionTitle title="最近结果" subtitle="同步和导入完成后会在这里显示" />
        {results.length ? (
          <div className="result-list">
            {results.slice(0, 5).map((result) => (
              <ResultRow key={`${result.agentId}:${result.skillId}:${result.targetPath}`} result={result} compact />
            ))}
          </div>
        ) : (
          <EmptyState title="暂无记录" body="完成一次导入或同步后，会显示操作结果。" />
        )}
      </section>
    </div>
  );
}

function SkillsView({
  skills,
  selectedSkills,
  dragging,
  busy,
  onToggle,
  onDrop,
  onDrag,
  onFolder,
  onArchive,
}: {
  skills: SkillSummary[];
  selectedSkills: string[];
  dragging: boolean;
  busy: boolean;
  onToggle: (id: string) => void;
  onDrop: (event: DragEvent<HTMLElement>) => void;
  onDrag: (dragging: boolean) => void;
  onFolder: () => void;
  onArchive: () => void;
}) {
  return (
    <div className="view-stack">
      <section
        className={`drop-panel ${dragging ? "dragging" : ""}`}
        onDragOver={(event) => {
          event.preventDefault();
          onDrag(true);
        }}
        onDragLeave={() => onDrag(false)}
        onDrop={onDrop}
      >
        <UploadCloud size={22} />
        <div>
          <strong>拖拽文件夹或 zip 到这里导入</strong>
          <span>也可以使用右侧按钮选择本地文件。</span>
        </div>
        <button className="secondary-button" onClick={onFolder} disabled={busy} type="button">
          <FolderPlus size={16} />
          文件夹
        </button>
        <button className="secondary-button" onClick={onArchive} disabled={busy} type="button">
          <Archive size={16} />
          zip
        </button>
      </section>

      <section className="panel-section fill">
        <SectionTitle title="Skills" subtitle={`${skills.length} 个可见项目`} />
        <div className="item-list">
          {skills.map((skill) => (
            <button
              className={`item-row ${selectedSkills.includes(skill.manifest.id) ? "selected" : ""}`}
              key={skill.manifest.id}
              onClick={() => onToggle(skill.manifest.id)}
              type="button"
            >
              <div>
                <strong>{skill.manifest.name}</strong>
                <span>{skill.manifest.description || skill.manifest.id}</span>
                <div className="chip-line">
                  <small>v{skill.manifest.version}</small>
                  <small>{skill.manifest.supportedAgents.join(", ") || "未声明 Agent"}</small>
                </div>
              </div>
              <Check className="checkmark" size={17} />
            </button>
          ))}
          {!skills.length && <EmptyState title="没有找到 skills" body="设置主仓库或导入包含 manifest 的文件夹后会显示在这里。" />}
        </div>
      </section>
    </div>
  );
}

function AgentsView({
  agents,
  selectedAgents,
  states,
  customAgent,
  busy,
  onToggle,
  onCustomChange,
  onSaveCustom,
}: {
  agents: AgentProfile[];
  selectedAgents: string[];
  states: InstallState[];
  customAgent: AgentProfile;
  busy: boolean;
  onToggle: (id: string) => void;
  onCustomChange: (agent: AgentProfile) => void;
  onSaveCustom: () => void;
}) {
  return (
    <div className="view-stack two-column">
      <section className="panel-section fill">
        <SectionTitle title="Agents" subtitle={`${agents.length} 个可同步目标`} />
        <div className="item-list">
          {agents.map((agent) => (
            <button
              className={`item-row ${selectedAgents.includes(agent.id) ? "selected" : ""}`}
              key={agent.id}
              onClick={() => onToggle(agent.id)}
              type="button"
            >
              <div>
                <strong>{agentLabel(agent)}</strong>
                <span>{agent.skillsPath}</span>
                <div className="chip-line">
                  <small>{states.filter((state) => state.agentId === agent.id && state.status === "installed").length} 已安装</small>
                  <small>{states.filter((state) => state.agentId === agent.id && state.status === "stale").length} 需更新</small>
                </div>
              </div>
              <Check className="checkmark" size={17} />
            </button>
          ))}
          {!agents.length && <EmptyState title="没有发现 Agent" body="可以添加自定义 Agent，把普通目录作为同步目标。" />}
        </div>
      </section>

      <section className="panel-section">
        <SectionTitle title="自定义 Agent" subtitle="添加一个本地目录作为同步目标" />
        <div className="form-stack">
          <label>
            <span>名称</span>
            <input value={customAgent.name} onChange={(event) => onCustomChange({ ...customAgent, name: event.target.value })} placeholder="例如 My Agent" />
          </label>
          <label>
            <span>Skills 安装目录</span>
            <input value={customAgent.skillsPath} onChange={(event) => onCustomChange({ ...customAgent, skillsPath: event.target.value })} placeholder="C:\\Users\\you\\.agent\\skills" />
          </label>
          <button className="primary-button" onClick={onSaveCustom} disabled={busy} type="button">
            <Check size={16} />
            添加 Agent
          </button>
        </div>
      </section>
    </div>
  );
}

function SyncView({
  selectedSkills,
  selectedAgents,
  agents,
  stateByPair,
  conflictPolicy,
  results,
  busy,
  onPolicy,
  onInstall,
  onRefresh,
}: {
  selectedSkills: string[];
  selectedAgents: string[];
  agents: AgentProfile[];
  stateByPair: Map<string, InstallState>;
  conflictPolicy: ConflictPolicy;
  results: InstallResult[];
  busy: boolean;
  onPolicy: (policy: ConflictPolicy) => void;
  onInstall: () => void;
  onRefresh: () => void;
}) {
  return (
    <div className="view-stack">
      <section className="panel-section">
        <SectionTitle title="冲突策略" subtitle="选择同步时遇到冲突的处理方式" />
        <div className="policy-grid">
          {policyOptions.map((option) => (
            <button className={`policy-card ${conflictPolicy === option.value ? "active" : ""}`} key={option.value} onClick={() => onPolicy(option.value)} type="button">
              <strong>{option.label}</strong>
              <span>{option.helper}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="panel-section fill">
        <div className="section-title-row">
          <SectionTitle title="同步矩阵" subtitle={`${selectedSkills.length} x ${selectedAgents.length} 个组合`} />
          <button className="primary-button" onClick={onInstall} disabled={busy || !selectedSkills.length || !selectedAgents.length} type="button">
            <RefreshCw size={16} />
            执行同步
          </button>
        </div>
        <div className="matrix-list">
          {selectedSkills.map((skillId) =>
            selectedAgents.map((agentId) => {
              const state = stateByPair.get(`${agentId}:${skillId}`);
              return (
                <div className={`matrix-row ${state?.status ?? "missing"}`} key={`${agentId}:${skillId}`}>
                  <span>{skillId}</span>
                  <span>{agents.find((agent) => agent.id === agentId)?.name ?? agentId}</span>
                  <strong>{statusText(state?.status)}</strong>
                  {state?.status === "conflict" && <ShieldAlert size={15} />}
                </div>
              );
            }),
          )}
          {(!selectedSkills.length || !selectedAgents.length) && <EmptyState title="还没有同步矩阵" body="请先选择至少一个 skill 和一个 agent。" />}
        </div>
      </section>

      <section className="panel-section">
        <SectionTitle title="最近结果" subtitle="安装、更新、跳过和回滚记录" />
        <div className="result-list">
          {results.map((result) => (
            <ResultRow key={`${result.agentId}:${result.skillId}:${result.targetPath}`} result={result} onRefresh={onRefresh} />
          ))}
          {!results.length && <EmptyState title="暂无同步结果" body="执行同步后会显示详细记录。" />}
        </div>
      </section>
    </div>
  );
}

function SettingsView({
  repository,
  busy,
  onRepository,
  onSave,
}: {
  repository: string;
  busy: boolean;
  onRepository: (path: string) => void;
  onSave: () => void;
}) {
  return (
    <div className="view-stack">
      <section className="panel-section">
        <SectionTitle title="主仓库" subtitle="Skills Manager 会从这里扫描和导入 skills" />
        <div className="repo-editor">
          <input value={repository} onChange={(event) => onRepository(event.target.value)} placeholder="C:\\Users\\you\\skills" />
          <button className="primary-button" onClick={onSave} disabled={busy} type="button">
            <FolderPlus size={16} />
            保存并扫描
          </button>
        </div>
      </section>

      <section className="panel-section">
        <SectionTitle title="关于" subtitle="当前默认界面为 Tauri + React WebView" />
        <div className="detail-list">
          <Detail label="UI" value="Tauri / React / CSS" />
          <Detail label="窗口" value="透明无边框，自绘标题栏" />
          <Detail label="版本" value="0.1.0" />
        </div>
      </section>
    </div>
  );
}

function RightPanel({
  skills,
  agents,
  selectedSkill,
  selectedAgent,
  selectedSkills,
  selectedAgents,
  installedCount,
  conflictCount,
  conflictPolicy,
  onPolicy,
  onInstall,
  busy,
}: {
  skills: SkillSummary[];
  agents: AgentProfile[];
  selectedSkill?: SkillSummary;
  selectedAgent?: AgentProfile;
  selectedSkills: string[];
  selectedAgents: string[];
  installedCount: number;
  conflictCount: number;
  conflictPolicy: ConflictPolicy;
  onPolicy: (policy: ConflictPolicy) => void;
  onInstall: () => void;
  busy: boolean;
}) {
  return (
    <aside className="detail-panel">
      <div className="detail-heading">
        <PanelRight size={18} />
        <strong>详情</strong>
      </div>

      <div className="detail-list">
        <Detail label="Skills" value={String(skills.length)} />
        <Detail label="Agents" value={String(agents.length)} />
        <Detail label="已安装" value={String(installedCount)} />
        <Detail label="冲突" value={String(conflictCount)} />
      </div>

      <div className="detail-block">
        <span>当前选择</span>
        <strong>{selectedSkills.length} 个 skill / {selectedAgents.length} 个 agent</strong>
      </div>

      <div className="detail-block">
        <span>Skill</span>
        <strong>{selectedSkill?.manifest.name ?? "未选择"}</strong>
        {selectedSkill && <p>{selectedSkill.manifest.description || selectedSkill.manifest.id}</p>}
      </div>

      <div className="detail-block">
        <span>Agent</span>
        <strong>{selectedAgent?.name ?? "未选择"}</strong>
        {selectedAgent && <p>{selectedAgent.skillsPath}</p>}
      </div>

      <div className="mini-policy">
        {policyOptions.map((option) => (
          <button key={option.value} className={conflictPolicy === option.value ? "active" : ""} onClick={() => onPolicy(option.value)} type="button">
            {option.label}
          </button>
        ))}
      </div>

      <button className="primary-button wide" onClick={onInstall} disabled={busy || !selectedSkills.length || !selectedAgents.length} type="button">
        <RefreshCw size={16} />
        同步选中
      </button>
    </aside>
  );
}

function Metric({ label, value, helper }: { label: string; value: number; helper: string }) {
  return (
    <div className="metric-card">
      <strong>{value}</strong>
      <span>{label}</span>
      <small>{helper}</small>
    </div>
  );
}

function SectionTitle({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="section-title">
      <h2>{title}</h2>
      <p>{subtitle}</p>
    </div>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="detail-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="empty-state">
      <strong>{title}</strong>
      <span>{body}</span>
    </div>
  );
}

function ResultRow({ result, compact, onRefresh }: { result: InstallResult; compact?: boolean; onRefresh?: () => void }) {
  async function rollback() {
    await api.rollbackLast(result.agentId, result.skillId);
    onRefresh?.();
  }

  async function uninstall() {
    await api.uninstallSkill(result.skillId, result.agentId);
    onRefresh?.();
  }

  return (
    <div className="result-row">
      <span>{actionText(result.action)}</span>
      <p>{result.message}</p>
      {!compact && (
        <>
          <button title="回滚最近一次变更" onClick={rollback} type="button">
            <RotateCcw size={15} />
          </button>
          <button title="卸载" onClick={uninstall} type="button">
            <Trash2 size={15} />
          </button>
        </>
      )}
      {compact && <ChevronRight size={15} />}
    </div>
  );
}
