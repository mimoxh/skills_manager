import { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import type { AgentProfile, ConflictPolicy, GroupedSkill, InstallResult } from "../../types";

const policyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新目标目录" },
  { value: "skip", label: "跳过冲突", helper: "目标已存在时不做修改" },
  { value: "rename", label: "另存副本", helper: "生成带时间戳的新副本" },
];

interface SkillsViewProps {
  skills: GroupedSkill[];
  agents: AgentProfile[];
  busy: boolean;
  onDrop: (event: React.DragEvent<HTMLElement>) => void;
  onFolder: () => void;
  onArchive: () => void;
  onSync: (title: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<InstallResult[]>;
  onUninstall: (skillId: string, agentIds: string[]) => Promise<void>;
  onRefresh: () => void;
}

export function SkillsView({ skills, agents, busy, onDrop, onFolder, onArchive, onSync, onUninstall, onRefresh }: SkillsViewProps) {
  const [selectedSkill, setSelectedSkill] = useState<GroupedSkill | null>(null);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");
  const [lastResults, setLastResults] = useState<InstallResult[]>([]);
  const [dragging, setDragging] = useState(false);
  const [filter, setFilter] = useState<"all" | "missing">("all");
  const [deleteTarget, setDeleteTarget] = useState<GroupedSkill | null>(null);

  const displayedSkills = useMemo(() => {
    if (filter === "missing") return skills.filter((s) => s.missingAgentIds.length > 0);
    return skills;
  }, [skills, filter]);

  const incompleteCount = useMemo(() => {
    return skills.filter((s) => s.missingAgentIds.length > 0).length;
  }, [skills]);

  function openSync(skill: GroupedSkill) {
    setSelectedSkill(skill);
    setSelectedAgents(skill.installedAgentIds);
    setConflictPolicy("backupOverwrite");
    setLastResults([]);
  }

  function toggleAgent(agentId: string) {
    setSelectedAgents((prev) =>
      prev.includes(agentId) ? prev.filter((id) => id !== agentId) : [...prev, agentId],
    );
  }

  async function executeSync() {
    if (!selectedSkill) return;
    const deselectedIds = selectedSkill.installedAgentIds.filter((id) => !selectedAgents.includes(id));
    if (deselectedIds.length > 0) {
      await onUninstall(selectedSkill.title, deselectedIds);
    }
    if (selectedAgents.length > 0) {
      const results = await onSync(selectedSkill.title, selectedAgents, conflictPolicy);
      setLastResults(results);
    }
    setSelectedSkill(null);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    await onUninstall(deleteTarget.title, deleteTarget.installedAgentIds);
    setDeleteTarget(null);
  }

  return (
    <>
      {/* Metrics */}
      <div className="metrics">
        <div className="metric-card"><div className="metric-value">{skills.length}</div><div className="metric-label">Skills</div></div>
        <div
          className="metric-card"
          onClick={() => setFilter((f) => f === "missing" ? "all" : "missing")}
          style={{ cursor: "pointer", ...(filter === "missing" ? { borderColor: "var(--warning)", background: "var(--warning-light)" } : {}) }}
        >
          <div className="metric-value warning">{incompleteCount}</div>
          <div className="metric-label">未全覆盖</div>
        </div>
      </div>

      {/* Import Zone */}
      <div
        className="import-zone"
        onDragOver={(e) => { e.preventDefault(); setDragging(true); }}
        onDragLeave={() => setDragging(false)}
        onDrop={onDrop}
        style={dragging ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : undefined}
      >
        <div className="import-zone-icon">
          <svg className="icon icon-lg" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="17 8 12 3 7 8" /><line x1="12" y1="3" x2="12" y2="15" /></svg>
        </div>
        <div className="import-zone-text">
          <div className="import-zone-title">导入 Skill</div>
          <div className="import-zone-desc">拖拽文件夹或 zip 到这里，或选择本地文件</div>
        </div>
        <button className="btn btn-secondary btn-sm" onClick={onFolder} disabled={busy} type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
          文件夹
        </button>
        <button className="btn btn-secondary btn-sm" onClick={onArchive} disabled={busy} type="button">
          <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>
          zip
        </button>
      </div>

      {/* Skills Panel */}
      <div className="skills-panel">
        <div className="skills-header">
          <div>
            <div className="card-title">Skills 控制台</div>
            <div className="card-desc">点击任意 skill 选择目标 Agent 同步</div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span className="skills-header-badge">{displayedSkills.length} 个可见项目</span>
            <button className="btn btn-secondary btn-sm" onClick={onRefresh} disabled={busy} type="button" title="刷新">
              <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
            </button>
          </div>
        </div>
        <div className="skills-list">
          {displayedSkills.map((skill) => (
            <div
              key={skill.title}
              className="skill-item"
              onClick={() => openSync(skill)}
              role="button"
              tabIndex={0}
            >
              <div className="skill-icon">
                <svg className="icon" viewBox="0 0 24 24"><polygon points="12 2 2 7 12 12 22 7 12 2" /><polyline points="2 17 12 22 22 17" /><polyline points="2 12 12 17 22 12" /></svg>
              </div>
              <div className="skill-info">
                <div className="skill-name">{skill.title}</div>
                <div className="skill-meta">来源 {skill.bestCopy.agentName} · {skill.copies.length} 个副本</div>
                {skill.description && <div className="skill-desc">{skill.description}</div>}
                <div className="skill-tags">
                  <span className="badge badge-version">{skill.bestCopy.version ? `v${skill.bestCopy.version}` : "未声明版本"}</span>
                  <span className="badge badge-success">{skill.installedAgentIds.length} 已有</span>
                  {skill.missingAgentIds.length > 0 && (
                    <span className="badge badge-warning">{skill.missingAgentIds.length} 缺失</span>
                  )}
                </div>
              </div>
              <span className={`badge ${skill.missingAgentIds.length > 0 ? "badge-syncable" : "badge-synced"}`}>
                {skill.missingAgentIds.length > 0 ? "可同步" : "已覆盖"}
              </span>
              <button
                className="btn-icon danger"
                title="删除"
                style={{ width: 32, height: 32, flexShrink: 0 }}
                onClick={(e) => { e.stopPropagation(); setDeleteTarget(skill); }}
                disabled={busy}
                type="button"
              >
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
              </button>
            </div>
          ))}
          {!displayedSkills.length && (
            <div style={{ textAlign: "center", padding: "48px 0", color: "var(--text-secondary)" }}>
              <p style={{ fontSize: 14, fontWeight: 500 }}>没有找到 skills</p>
              <p style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: 4 }}>设置主仓库或导入包含 manifest 的文件夹后会显示在这里</p>
            </div>
          )}
        </div>
      </div>

      {/* Last results */}
      {lastResults.length > 0 && (
        <div style={{ marginTop: 16, padding: "12px 16px", background: "var(--surface-raised)", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", fontSize: 12, color: "var(--text-secondary)" }}>
          最近同步完成 {lastResults.length} 个任务
        </div>
      )}

      {/* Sync Dialog */}
      {selectedSkill && (
        <SyncSkillDialog
          agents={agents}
          busy={busy}
          conflictPolicy={conflictPolicy}
          selectedAgents={selectedAgents}
          skill={selectedSkill}
          onClose={() => setSelectedSkill(null)}
          onPolicy={setConflictPolicy}
          onSync={executeSync}
          onToggleAgent={toggleAgent}
        />
      )}

      {/* Delete Confirm Dialog */}
      {deleteTarget && (
        <ConfirmDialog
          title="删除 Skill"
          message={`确定要从所有 ${deleteTarget.installedAgentIds.length} 个 Agent 中删除 "${deleteTarget.title}" 吗？`}
          confirmLabel="删除"
          busy={busy}
          onClose={() => setDeleteTarget(null)}
          onConfirm={confirmDelete}
        />
      )}
    </>
  );
}

function SyncSkillDialog({
  agents, busy, conflictPolicy, selectedAgents, skill,
  onClose, onPolicy, onSync, onToggleAgent,
}: {
  agents: AgentProfile[];
  busy: boolean;
  conflictPolicy: ConflictPolicy;
  selectedAgents: string[];
  skill: GroupedSkill;
  onClose: () => void;
  onPolicy: (policy: ConflictPolicy) => void;
  onSync: () => void;
  onToggleAgent: (agentId: string) => void;
}) {
  const readmeContent = skill.readme || skill.description;

  return (
    <div style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div style={{ maxHeight: "88vh", width: "100%", maxWidth: 960, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>{skill.title}</h2>
            <p style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>来源 {skill.bestCopy.agentName} · {skill.copies.length} 个副本</p>
          </div>
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        {/* Body - Two Column Layout */}
        <div style={{ flex: 1, overflow: "hidden", display: "grid", gridTemplateColumns: "1.2fr 1fr" }}>
          {/* Left Column - Markdown Reader */}
          <div style={{ overflow: "auto", padding: "20px 24px", borderRight: "1px solid var(--border)" }}>
            <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 12 }}>Skill 说明</p>
            {readmeContent ? (
              <div className="markdown-body">
                <ReactMarkdown>{readmeContent}</ReactMarkdown>
              </div>
            ) : (
              <p style={{ fontSize: 13, color: "var(--text-tertiary)", fontStyle: "italic" }}>暂无说明</p>
            )}
          </div>

          {/* Right Column - Agent Sync */}
          <div style={{ overflow: "auto", padding: "20px 24px", display: "flex", flexDirection: "column", gap: 20 }}>
            {/* Agents */}
            <div>
              <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>目标 Agent</p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {agents.map((agent) => {
                  const checked = selectedAgents.includes(agent.id);
                  const installed = skill.installedAgentIds.includes(agent.id);
                  return (
                    <button
                      key={agent.id}
                      className={`agent-item${checked ? " selected" : ""}`}
                      onClick={() => onToggleAgent(agent.id)}
                      type="button"
                      style={checked ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : undefined}
                    >
                      <span style={{
                        width: 20, height: 20, flexShrink: 0, borderRadius: 4, border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                        background: checked ? "var(--accent)" : "transparent", display: "flex", alignItems: "center", justifyContent: "center",
                      }}>
                        {checked && <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3"><polyline points="20 6 9 17 4 12" /></svg>}
                      </span>
                      <span style={{ flex: 1, minWidth: 0 }}>
                        <span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{agent.name}</span>
                        <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{agent.skillsPath}</span>
                      </span>
                      <span className={`badge ${installed ? "badge-success" : "badge-warning"}`}>{installed ? "已安装" : "未安装"}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Conflict Policy */}
            <div>
              <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>冲突策略</p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {policyOptions.map((option) => (
                  <button
                    key={option.value}
                    className={`card${conflictPolicy === option.value ? " selected" : ""}`}
                    onClick={() => onPolicy(option.value)}
                    type="button"
                    style={{
                      padding: 12, textAlign: "left", cursor: "pointer",
                      ...(conflictPolicy === option.value ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : {}),
                    }}
                  >
                    <span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{option.label}</span>
                    <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{option.helper}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, borderTop: "1px solid var(--border)", background: "var(--surface-raised)", padding: "16px 24px" }}>
          <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>已选择 {selectedAgents.length} 个 Agent</p>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-secondary" onClick={onClose} disabled={busy} type="button">取消</button>
            {selectedAgents.length === 0 ? (
              <button className="btn btn-danger" onClick={onSync} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
                全部删除
              </button>
            ) : (
              <button className="btn btn-primary" onClick={onSync} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
                {selectedAgents.length < skill.installedAgentIds.length ? "同步并清理" : "同步"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function ConfirmDialog({
  title, message, confirmLabel, busy,
  onClose, onConfirm,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  busy: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <div style={{ position: "fixed", inset: 0, zIndex: 60, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.36)", padding: 20 }}>
      <div style={{ width: "100%", maxWidth: 420, borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14)" }}>
        <div style={{ padding: "20px 24px" }}>
          <h3 style={{ fontSize: 15, fontWeight: 600, color: "var(--text)", marginBottom: 8 }}>{title}</h3>
          <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>{message}</p>
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "14px 24px", borderTop: "1px solid var(--border)" }}>
          <button className="btn btn-secondary" onClick={onClose} disabled={busy} type="button">取消</button>
          <button className="btn btn-danger" onClick={onConfirm} disabled={busy} type="button">{confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}

export function ImportAgentDialog({
  agents, busy, fileName,
  onClose, onImport,
}: {
  agents: AgentProfile[];
  busy: boolean;
  fileName: string;
  onClose: () => void;
  onImport: (targetAgentIds: string[], conflictPolicy: ConflictPolicy) => void;
}) {
  const [selectedAgents, setSelectedAgents] = useState<string[]>(agents.map((a) => a.id));
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");

  function toggleAgent(agentId: string) {
    setSelectedAgents((prev) =>
      prev.includes(agentId) ? prev.filter((id) => id !== agentId) : [...prev, agentId],
    );
  }

  function toggleAll() {
    if (selectedAgents.length === agents.length) {
      setSelectedAgents([]);
    } else {
      setSelectedAgents(agents.map((a) => a.id));
    }
  }

  return (
    <div style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div style={{ maxHeight: "88vh", width: "100%", maxWidth: 560, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="17 8 12 3 7 8" /><line x1="12" y1="3" x2="12" y2="15" /></svg>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>选择导入目标</h2>
            <p style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{fileName}</p>
          </div>
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px 24px", display: "flex", flexDirection: "column", gap: 20 }}>
          {/* Agents */}
          <div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
              <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>目标 Agent</p>
              <button className="btn btn-secondary btn-sm" onClick={toggleAll} type="button" style={{ fontSize: 11, padding: "2px 8px" }}>
                {selectedAgents.length === agents.length ? "取消全选" : "全选"}
              </button>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {agents.map((agent) => {
                const checked = selectedAgents.includes(agent.id);
                return (
                  <button
                    key={agent.id}
                    className={`agent-item${checked ? " selected" : ""}`}
                    onClick={() => toggleAgent(agent.id)}
                    type="button"
                    style={checked ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : undefined}
                  >
                    <span style={{
                      width: 20, height: 20, flexShrink: 0, borderRadius: 4, border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                      background: checked ? "var(--accent)" : "transparent", display: "flex", alignItems: "center", justifyContent: "center",
                    }}>
                      {checked && <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3"><polyline points="20 6 9 17 4 12" /></svg>}
                    </span>
                    <span style={{ flex: 1, minWidth: 0 }}>
                      <span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{agent.name}</span>
                      <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{agent.skillsPath}</span>
                    </span>
                  </button>
                );
              })}
            </div>
          </div>

          {/* Conflict Policy */}
          <div>
            <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>冲突策略</p>
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {policyOptions.map((option) => (
                <button
                  key={option.value}
                  className={`card${conflictPolicy === option.value ? " selected" : ""}`}
                  onClick={() => setConflictPolicy(option.value)}
                  type="button"
                  style={{
                    padding: 12, textAlign: "left", cursor: "pointer",
                    ...(conflictPolicy === option.value ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : {}),
                  }}
                >
                  <span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{option.label}</span>
                  <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{option.helper}</span>
                </button>
              ))}
            </div>
          </div>
        </div>

        {/* Footer */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, borderTop: "1px solid var(--border)", background: "var(--surface-raised)", padding: "16px 24px" }}>
          <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>已选择 {selectedAgents.length} 个 Agent</p>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn btn-secondary" onClick={onClose} disabled={busy} type="button">取消</button>
            <button className="btn btn-primary" onClick={() => onImport(selectedAgents, conflictPolicy)} disabled={busy || selectedAgents.length === 0} type="button">
              <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="17 8 12 3 7 8" /><line x1="12" y1="3" x2="12" y2="15" /></svg>
              导入
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
