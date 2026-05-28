import { useState } from "react";
import type { AgentProfile, GroupedSkill } from "../../types";

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
}

export function AgentsView({ agents, skills, customAgent, busy, onCustomChange, onSaveCustom, onDelete }: AgentsViewProps) {
  const [detailAgentId, setDetailAgentId] = useState<string | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const detailAgent = detailAgentId ? agents.find((a) => a.id === detailAgentId) : null;
  const installedSkills = detailAgent ? skills.filter((s) => s.installedAgentIds.includes(detailAgent.id)) : [];
  const missingSkills = detailAgent ? skills.filter((s) => s.missingAgentIds.includes(detailAgent.id)) : [];

  function handleAgentClick(agentId: string) {
    setDetailAgentId((prev) => (prev === agentId ? null : agentId));
    setConfirmDeleteId(null);
  }

  function handleDelete(e: React.MouseEvent, agentId: string) {
    e.stopPropagation();
    if (confirmDeleteId === agentId) {
      onDelete(agentId);
      setConfirmDeleteId(null);
      setDetailAgentId(null);
    } else {
      setConfirmDeleteId(agentId);
    }
  }

  return (
    <div className="grid-main-side" style={{ height: "100%", minHeight: 0 }}>
      {/* Agent List */}
      <div className="card flex-col" style={{ minHeight: 0 }}>
        <div className="card-header">
          <div className="card-title">Agents</div>
          <div className="card-desc">{agents.length} 个本地 Agent 配置</div>
        </div>
        <div className="card-body flex-1 overflow-auto" style={{ padding: 8 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            {agents.map((agent) => {
              const isDetail = detailAgentId === agent.id;
              const isConfirming = confirmDeleteId === agent.id;
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
                    className={`btn-icon${isConfirming ? " danger" : ""}`}
                    onClick={(e) => handleDelete(e, agent.id)}
                    disabled={busy}
                    title={isConfirming ? "确认删除" : "删除"}
                    type="button"
                    style={isConfirming ? { background: "var(--danger-light)", color: "var(--danger)" } : undefined}
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
      <div className="flex-col gap-5" style={{ height: "100%", minHeight: 0, overflowY: "auto" }}>
        {detailAgent ? (
          <div className="card">
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
            <div className="card-body">
              <div className="detail-section">
                <div className="detail-section-title">已安装 ({installedSkills.length})</div>
                {installedSkills.map((s) => (
                  <div key={s.title} className="detail-item">
                    <span className="detail-item-name">{s.title}</span>
                    <span className="badge badge-version">{s.bestCopy.version ? `v${s.bestCopy.version}` : "-"}</span>
                  </div>
                ))}
                {!installedSkills.length && <p style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "8px 0" }}>暂无已安装 skills</p>}
              </div>
              {missingSkills.length > 0 && (
                <div className="detail-section">
                  <div className="detail-section-title">缺失 ({missingSkills.length})</div>
                  {missingSkills.map((s) => (
                    <div key={s.title} className="detail-item" style={{ borderStyle: "dashed", color: "var(--text-secondary)" }}>
                      <span className="detail-item-name">{s.title}</span>
                      <span className="badge badge-warning">缺失</span>
                    </div>
                  ))}
                </div>
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
    </div>
  );
}
