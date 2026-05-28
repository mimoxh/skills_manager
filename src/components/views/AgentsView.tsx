import { useState } from "react";
import type { AgentProfile, ConflictPolicy, GroupedSkill, InstallResult } from "../../types";

async function pickFolder(): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}

interface AgentsViewProps {
  agents: AgentProfile[];
  skills: GroupedSkill[];
  customAgent: AgentProfile;
  busy: boolean;
  onCustomChange: (agent: AgentProfile) => void;
  onSaveCustom: () => void;
  onDelete: (agentId: string) => void;
  onSync: (title: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<InstallResult[]>;
  onRefresh: () => void;
}

export function AgentsView({ agents, skills, customAgent, busy, onCustomChange, onSaveCustom, onDelete, onSync, onRefresh }: AgentsViewProps) {
  const [detailAgentId, setDetailAgentId] = useState<string | null>(null);
  const [deleteDialogAgent, setDeleteDialogAgent] = useState<AgentProfile | null>(null);
  const [selectedMissing, setSelectedMissing] = useState<string[]>([]);

  const detailAgent = detailAgentId ? agents.find((a) => a.id === detailAgentId) : null;
  const installedSkills = detailAgent ? skills.filter((s) => s.installedAgentIds.includes(detailAgent.id)) : [];
  const missingSkills = detailAgent ? skills.filter((s) => s.missingAgentIds.includes(detailAgent.id)) : [];

  function handleAgentClick(agentId: string) {
    setDetailAgentId((prev) => (prev === agentId ? null : agentId));
    setSelectedMissing([]);
  }

  function handleDeleteClick(e: React.MouseEvent, agent: AgentProfile) {
    e.stopPropagation();
    setDeleteDialogAgent(agent);
  }

  function confirmDelete() {
    if (!deleteDialogAgent) return;
    onDelete(deleteDialogAgent.id);
    setDeleteDialogAgent(null);
    setDetailAgentId(null);
  }

  function toggleMissing(title: string) {
    setSelectedMissing((prev) => prev.includes(title) ? prev.filter((t) => t !== title) : [...prev, title]);
  }

  async function handleAddMissing() {
    if (!detailAgent || !selectedMissing.length) return;
    for (const title of selectedMissing) {
      await onSync(title, [detailAgent.id], "prompt");
    }
    setSelectedMissing([]);
  }

  return (
    <div className="grid-main-side" style={{ height: "100%", minHeight: 0 }}>
      {/* Agent List */}
      <div className="card flex-col" style={{ minHeight: 0 }}>
        <div className="card-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="card-title">Agents</div>
            <div className="card-desc">{agents.length} 个本地 Agent 配置</div>
          </div>
          <button className="btn btn-secondary btn-sm" onClick={onRefresh} disabled={busy} type="button" title="刷新">
            <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
          </button>
        </div>
        <div className="card-body flex-1 overflow-auto" style={{ padding: 8 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {agents.map((agent) => {
              const isDetail = detailAgentId === agent.id;
              const installedCount = skills.filter((s) => s.installedAgentIds.includes(agent.id)).length;
              const missingCount = skills.filter((s) => s.missingAgentIds.includes(agent.id)).length;
              return (
                <div
                  key={agent.id}
                  className={`agent-item${isDetail ? " selected" : ""}`}
                  onClick={() => handleAgentClick(agent.id)}
                  role="button"
                  tabIndex={0}
                >
                  <div className="agent-icon">
                    <svg className="icon" viewBox="0 0 24 24"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>
                  </div>
                  <div className="agent-info">
                    <div className="agent-name">{agent.name}</div>
                    <div className="agent-path">{agent.skillsPath}</div>
                    <div className="agent-tags">
                      <span className="badge badge-success">{installedCount} 已有</span>
                      {missingCount > 0 && <span className="badge badge-warning">{missingCount} 缺失</span>}
                    </div>
                  </div>
                  <button
                    className="btn-icon"
                    onClick={(e) => handleDeleteClick(e, agent)}
                    disabled={busy}
                    title="删除"
                    type="button"
                  >
                    <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
                  </button>
                </div>
              );
            })}
            {!agents.length && (
              <div style={{ textAlign: "center", padding: "40px 0", color: "var(--text-secondary)" }}>
                <p style={{ fontSize: 14, fontWeight: 500 }}>没有发现 Agent</p>
                <p style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: 4 }}>可以添加自定义 Agent，把普通目录纳入管理</p>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Side Panel */}
      <div style={{ height: "100%", minHeight: 0, display: "flex" }}>
        {detailAgent ? (
          <div className="card" style={{ display: "flex", flexDirection: "column", height: "100%", overflow: "hidden" }}>
            <div className="card-header">
              <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                <div className="agent-icon">
                  <svg className="icon" viewBox="0 0 24 24"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>
                </div>
                <div style={{ minWidth: 0 }}>
                  <div className="card-title" style={{ fontSize: 14 }}>{detailAgent.name}</div>
                  <div className="card-desc" style={{ marginTop: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{detailAgent.skillsPath}</div>
                </div>
              </div>
            </div>
            <div className="card-body" style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0, padding: "18px 24px" }}>
              <div className="detail-section-title">已安装 ({installedSkills.length})</div>
              <div style={{ flex: 1, overflowY: "auto", minHeight: 0 }}>
                {installedSkills.map((s) => (
                  <div key={s.title} className="detail-item">
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <span className="detail-item-name">{s.title}</span>
                      {s.description && <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: 2, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{s.description}</div>}
                    </div>
                    <span className="badge badge-version">{s.bestCopy.version ? `v${s.bestCopy.version}` : "-"}</span>
                  </div>
                ))}
                {!installedSkills.length && <p style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "8px 0" }}>暂无已安装 skills</p>}
              </div>
              {missingSkills.length > 0 && (
                <>
                  <div className="detail-section-title" style={{ marginTop: 12 }}>缺失 ({missingSkills.length})</div>
                  <div style={{ overflowY: "auto", minHeight: 0, maxHeight: 200 }}>
                    {missingSkills.map((s) => {
                      const isSelected = selectedMissing.includes(s.title);
                      return (
                        <div
                          key={s.title}
                          className={`detail-item${isSelected ? " selected" : ""}`}
                          style={{ borderStyle: "dashed", color: "var(--text-secondary)", cursor: "pointer" }}
                          onClick={() => toggleMissing(s.title)}
                          role="button"
                          tabIndex={0}
                        >
                          <div style={{ flex: 1, minWidth: 0 }}>
                            <span className="detail-item-name">{s.title}</span>
                            {s.description && <div style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: 2, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{s.description}</div>}
                          </div>
                          <span className={isSelected ? "badge badge-syncable" : "badge badge-warning"}>{isSelected ? "已选" : "缺失"}</span>
                        </div>
                      );
                    })}
                  </div>
                  {selectedMissing.length > 0 && (
                    <button
                      className="btn btn-primary"
                      onClick={handleAddMissing}
                      disabled={busy}
                      type="button"
                      style={{ marginTop: 8, width: "100%" }}
                    >
                      <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>
                      添加 {selectedMissing.length} 个 Skills
                    </button>
                  )}
                </>
              )}
            </div>
          </div>
        ) : (
          <div className="card">
            <div className="card-header">
              <div className="card-title">自定义 Agent</div>
              <div className="card-desc">添加一个本地目录作为 Agent 配置</div>
            </div>
            <div className="card-body">
              <div className="input-group">
                <label className="input-label">名称</label>
                <input
                  className="input"
                  value={customAgent.name}
                  onChange={(e) => onCustomChange({ ...customAgent, name: e.target.value })}
                  placeholder="例如 My Agent"
                />
              </div>
              <div className="input-group">
                <label className="input-label">Skills 安装目录</label>
                <div style={{ display: "flex", gap: 8 }}>
                  <input
                    className="input"
                    value={customAgent.skillsPath}
                    onChange={(e) => onCustomChange({ ...customAgent, skillsPath: e.target.value })}
                    placeholder="C:\Users\you\.agent\skills"
                    style={{ flex: 1 }}
                  />
                  <button
                    className="btn btn-secondary"
                    onClick={async () => {
                      const path = await pickFolder();
                      if (path) onCustomChange({ ...customAgent, skillsPath: path });
                    }}
                    disabled={busy}
                    type="button"
                  >
                    <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
                    浏览
                  </button>
                </div>
              </div>
              <button className="btn btn-primary" onClick={onSaveCustom} disabled={busy} type="button" style={{ marginTop: 4 }}>
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>
                添加 Agent
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Delete Confirm Dialog */}
      {deleteDialogAgent && (
        <div style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
          <div style={{ width: "100%", maxWidth: 420, borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)", overflow: "hidden" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "20px 24px", borderBottom: "1px solid var(--border)" }}>
              <div style={{ width: 40, height: 40, background: "var(--danger-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--danger)", flexShrink: 0 }}>
                <svg className="icon" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>确认删除</h2>
                <p style={{ fontSize: 13, color: "var(--text-secondary)", marginTop: 4 }}>是否确定删除「{deleteDialogAgent.name}」？此操作不可撤销。</p>
              </div>
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "12px 24px", background: "var(--surface-raised)" }}>
              <button className="btn btn-secondary" onClick={() => setDeleteDialogAgent(null)} disabled={busy} type="button">取消</button>
              <button className="btn btn-primary" onClick={confirmDelete} disabled={busy} type="button" style={{ background: "var(--danger)", borderColor: "var(--danger)" }}>
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
                确认删除
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
