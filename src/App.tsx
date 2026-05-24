import { useEffect, useMemo, useState } from "react";
import { Check, FolderPlus, RefreshCw, RotateCcw, Settings, ShieldAlert, Trash2 } from "lucide-react";
import { api } from "./api";
import type { AgentProfile, ConflictPolicy, InstallResult, InstallState, SkillSummary } from "./types";

const emptyCustom: AgentProfile = {
  id: "",
  name: "",
  type: "custom",
  skillsPath: "",
  adapterConfig: {},
};

function agentLabel(agent: AgentProfile) {
  return `${agent.name} (${agent.type})`;
}

export default function App() {
  const [repository, setRepository] = useState("");
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [agents, setAgents] = useState<AgentProfile[]>([]);
  const [states, setStates] = useState<InstallState[]>([]);
  const [selectedSkills, setSelectedSkills] = useState<string[]>([]);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [customAgent, setCustomAgent] = useState<AgentProfile>(emptyCustom);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("prompt");
  const [message, setMessage] = useState("选择一个本地 skills 主仓库开始。");
  const [results, setResults] = useState<InstallResult[]>([]);
  const [busy, setBusy] = useState(false);

  const stateByPair = useMemo(() => {
    const map = new Map<string, InstallState>();
    states.forEach((state) => map.set(`${state.agentId}:${state.skillId}`, state));
    return map;
  }, [states]);

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
      setAgents([...merged.values()]);
      setStates(installStates);
      setMessage(`已加载 ${scanned.length} 个 skills，${merged.size} 个 agent 配置。`);
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
      setMessage("主仓库已保存。");
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
      setMessage("自定义 agent 需要名称和 skills 目录。");
      return;
    }
    setBusy(true);
    try {
      await api.addAgent(agent);
      setCustomAgent(emptyCustom);
      await refreshAll();
      setSelectedAgents((prev) => [...new Set([...prev, agent.id])]);
      setMessage(`已添加 ${agent.name}，可选择兼容 skills 进行同步。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function installSelected() {
    if (!selectedSkills.length || !selectedAgents.length) {
      setMessage("请选择至少一个 skill 和一个 agent。");
      return;
    }
    if (conflictPolicy === "prompt") {
      const hasConflict = selectedSkills.some((skillId) =>
        selectedAgents.some((agentId) => stateByPair.get(`${agentId}:${skillId}`)?.status === "conflict"),
      );
      if (hasConflict) {
        setMessage("检测到冲突。请先选择覆盖、跳过或另存副本策略。");
        return;
      }
    }
    setBusy(true);
    try {
      const installResults = await api.installSkills(selectedSkills, selectedAgents, conflictPolicy);
      setResults(installResults);
      await refreshAll();
      setMessage(`完成 ${installResults.length} 个安装任务。`);
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

  return (
    <main className="app-shell">
      <header className="topbar">
        <div>
          <h1>Skills Manager</h1>
          <p>集中管理本地 skills，并同步到多个 agent 软件。</p>
        </div>
        <button className="icon-button" onClick={refreshAll} disabled={busy} title="刷新">
          <RefreshCw size={18} />
        </button>
      </header>

      <section className="repo-band">
        <label>
          <span>Skills 主仓库</span>
          <input value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="C:\\Users\\you\\skills" />
        </label>
        <button onClick={saveRepository} disabled={busy}>
          <FolderPlus size={17} />
          保存并扫描
        </button>
      </section>

      <section className="status-line" aria-live="polite">
        {busy ? "正在处理..." : message}
      </section>

      <div className="workspace">
        <section className="panel skills-panel">
          <div className="section-title">
            <h2>Skills</h2>
            <span>{skills.length}</span>
          </div>
          <div className="list">
            {skills.map((skill) => (
              <article
                className={`row-card ${selectedSkills.includes(skill.manifest.id) ? "selected" : ""}`}
                key={skill.manifest.id}
                onClick={() => toggleSkill(skill.manifest.id)}
              >
                <div>
                  <h3>{skill.manifest.name}</h3>
                  <p>{skill.manifest.description || skill.manifest.id}</p>
                  <div className="meta">
                    <span>v{skill.manifest.version}</span>
                    <span>{skill.manifest.supportedAgents.join(", ")}</span>
                  </div>
                </div>
                <input type="checkbox" checked={selectedSkills.includes(skill.manifest.id)} readOnly />
              </article>
            ))}
            {!skills.length && <div className="empty">主仓库里还没有可识别的 skill manifest。</div>}
          </div>
        </section>

        <section className="panel agents-panel">
          <div className="section-title">
            <h2>Agents</h2>
            <span>{agents.length}</span>
          </div>
          <div className="list">
            {agents.map((agent) => (
              <article
                className={`row-card ${selectedAgents.includes(agent.id) ? "selected" : ""}`}
                key={agent.id}
                onClick={() => toggleAgent(agent.id)}
              >
                <div>
                  <h3>{agentLabel(agent)}</h3>
                  <p>{agent.skillsPath}</p>
                  <div className="meta">
                    <span>{states.filter((state) => state.agentId === agent.id && state.status === "installed").length} installed</span>
                    <span>{states.filter((state) => state.agentId === agent.id && state.status === "stale").length} stale</span>
                  </div>
                </div>
                <input type="checkbox" checked={selectedAgents.includes(agent.id)} readOnly />
              </article>
            ))}
            {!agents.length && <div className="empty">没有发现 agent。可以手动添加自定义 agent。</div>}
          </div>

          <div className="custom-agent">
            <h3>
              <Settings size={16} />
              自定义 Agent
            </h3>
            <input value={customAgent.name} onChange={(event) => setCustomAgent({ ...customAgent, name: event.target.value })} placeholder="名称" />
            <input
              value={customAgent.skillsPath}
              onChange={(event) => setCustomAgent({ ...customAgent, skillsPath: event.target.value })}
              placeholder="Skills 安装目录"
            />
            <button onClick={saveCustomAgent} disabled={busy}>
              <Check size={16} />
              添加
            </button>
          </div>
        </section>

        <section className="panel sync-panel">
          <div className="section-title">
            <h2>同步</h2>
            <span>{selectedSkills.length} x {selectedAgents.length}</span>
          </div>

          <div className="policy-grid">
            {[
              ["prompt", "提示冲突"],
              ["backupOverwrite", "备份覆盖"],
              ["skip", "跳过冲突"],
              ["rename", "另存副本"],
            ].map(([value, label]) => (
              <button
                key={value}
                className={conflictPolicy === value ? "segmented active" : "segmented"}
                onClick={() => setConflictPolicy(value as ConflictPolicy)}
              >
                {label}
              </button>
            ))}
          </div>

          <button className="primary-action" onClick={installSelected} disabled={busy}>
            <RefreshCw size={18} />
            安装 / 更新选中项
          </button>

          <div className="matrix">
            {selectedSkills.map((skillId) =>
              selectedAgents.map((agentId) => {
                const state = stateByPair.get(`${agentId}:${skillId}`);
                return (
                  <div className={`matrix-row ${state?.status ?? "missing"}`} key={`${agentId}:${skillId}`}>
                    <span>{skillId}</span>
                    <span>{agents.find((agent) => agent.id === agentId)?.name ?? agentId}</span>
                    <strong>{state?.status ?? "missing"}</strong>
                    {state?.status === "conflict" && <ShieldAlert size={15} />}
                  </div>
                );
              }),
            )}
          </div>

          <div className="result-list">
            {results.map((result) => (
              <div className="result-row" key={`${result.agentId}:${result.skillId}:${result.targetPath}`}>
                <span>{result.action}</span>
                <p>{result.message}</p>
                <button title="回滚最近一次变更" onClick={() => api.rollbackLast(result.agentId, result.skillId).then(refreshAll)}>
                  <RotateCcw size={15} />
                </button>
                <button title="卸载" onClick={() => api.uninstallSkill(result.skillId, result.agentId).then(refreshAll)}>
                  <Trash2 size={15} />
                </button>
              </div>
            ))}
          </div>
        </section>
      </div>
    </main>
  );
}
