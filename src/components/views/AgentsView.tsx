import { useState } from "react";
import type { AgentProfile, AgentType, ConflictPolicy, GroupedSkill, InstallResult } from "../../types";

const agentTypeOptions: Array<{ value: AgentType; label: string }> = [
  { value: "custom", label: "自定义" },
  { value: "opencode", label: "OpenCode" },
  { value: "codex", label: "Codex" },
  { value: "claudeCode", label: "Claude Code" },
  { value: "claudeCowork", label: "Claude Desktop Cowork" },
  { value: "cursor", label: "Cursor" },
  { value: "trae", label: "Trae" },
];

const mcpAgentTypes: AgentType[] = ["codex", "claudeCode", "opencode", "trae"];
function isMcpAgent(type: AgentType): boolean { return mcpAgentTypes.includes(type); }

async function pickFolder(): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ directory: true, multiple: false });
  return typeof selected === "string" ? selected : null;
}

async function pickFile(filters?: Array<{ name: string; extensions: string[] }>): Promise<string | null> {
  if (!("__TAURI_INTERNALS__" in window)) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({ multiple: false, filters });
  return typeof selected === "string" ? selected : null;
}

interface AgentsViewProps {
  agents: AgentProfile[];
  skills: GroupedSkill[];
  customAgent: AgentProfile;
  busy: boolean;
  onCustomChange: (agent: AgentProfile) => void;
  onSaveCustom: () => void;
  onSaveAgent?: (agent: AgentProfile) => void;
  onDelete: (agentId: string) => void;
  onSync: (title: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<InstallResult[]>;
  onUninstall: (skillId: string, agentIds: string[]) => Promise<void>;
  onRepairCowork?: (agentId: string) => Promise<unknown>;
  onRefresh: () => void;
}

export function AgentsView({ agents, skills, customAgent, busy, onCustomChange, onSaveCustom, onSaveAgent, onDelete, onSync, onUninstall, onRepairCowork, onRefresh }: AgentsViewProps) {
  const [previewAgent, setPreviewAgent] = useState<AgentProfile | null>(null);
  const [editAgent, setEditAgent] = useState<AgentProfile | null>(null);
  const [deleteAgent, setDeleteAgent] = useState<AgentProfile | null>(null);
  const [selectedMissing, setSelectedMissing] = useState<string[]>([]);
  const [selectedInstalled, setSelectedInstalled] = useState<string[]>([]);
  const [deleteSkillsConfirm, setDeleteSkillsConfirm] = useState(false);

  const previewInstalled = previewAgent ? skills.filter((s) => s.installedAgentIds.includes(previewAgent.id)) : [];
  const previewMissing = previewAgent ? skills.filter((s) => s.missingAgentIds.includes(previewAgent.id)) : [];

  function handleAgentClick(agentId: string) {
    const agent = agents.find((a) => a.id === agentId);
    if (agent) {
      setPreviewAgent(agent);
      setSelectedMissing([]);
      setSelectedInstalled([]);
      setDeleteSkillsConfirm(false);
    }
  }

  function handleEditFromPreview() {
    if (!previewAgent) return;
    setEditAgent({ ...previewAgent });
    setPreviewAgent(null);
  }

  function handleSaveEdit() {
    if (!editAgent) return;
    if (onSaveAgent) {
      onSaveAgent(editAgent);
    } else {
      onCustomChange(editAgent);
      onSaveCustom();
    }
    setEditAgent(null);
  }

  async function handleAddMissing() {
    if (!previewAgent || !selectedMissing.length) return;
    for (const title of selectedMissing) { await onSync(title, [previewAgent.id], "prompt"); }
    setSelectedMissing([]);
  }

  async function handleDeleteInstalled() {
    if (!previewAgent || !selectedInstalled.length) return;
    for (const title of selectedInstalled) { await onUninstall(title, [previewAgent.id]); }
    setSelectedInstalled([]);
    setDeleteSkillsConfirm(false);
  }

  function closePreview() {
    setPreviewAgent(null);
    setSelectedMissing([]);
    setSelectedInstalled([]);
    setDeleteSkillsConfirm(false);
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
              const installedCount = skills.filter((s) => s.installedAgentIds.includes(agent.id)).length;
              const missingCount = skills.filter((s) => s.missingAgentIds.includes(agent.id)).length;
              return (
                <div key={agent.id} className="agent-item" onClick={() => handleAgentClick(agent.id)} role="button" tabIndex={0}>
                  <div className="agent-icon">
                    <svg className="icon" viewBox="0 0 24 24"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>
                  </div>
                  <div className="agent-info">
                    <div className="agent-name">{agent.name}<span className="badge" style={{ marginLeft: 6, fontSize: 10 }}>{agentTypeLabel(agent.type)}</span></div>
                    <div className="agent-path">{agent.skillsPath}</div>
                    <div className="agent-tags">
                      <span className="badge badge-success">{installedCount} 已有</span>
                      {missingCount > 0 && <span className="badge badge-warning">{missingCount} 缺失</span>}
                    </div>
                  </div>
                  <button className="btn-icon" onClick={(e) => { e.stopPropagation(); setDeleteAgent(agent); }} disabled={busy} title="删除" type="button">
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

      {/* Right Side: Add Agent Panel (always visible) */}
      <AddAgentPanel customAgent={customAgent} busy={busy} onCustomChange={onCustomChange} onSaveCustom={onSaveCustom} pickFolder={pickFolder} pickFile={pickFile} />

      {/* Preview Dialog */}
      {previewAgent && (
        <AgentPreviewDialog
          agent={previewAgent}
          installedSkills={previewInstalled}
          missingSkills={previewMissing}
          selectedMissing={selectedMissing}
          selectedInstalled={selectedInstalled}
          busy={busy}
          onClose={closePreview}
          onEdit={handleEditFromPreview}
          onDelete={() => { setDeleteAgent(previewAgent); setPreviewAgent(null); }}
          onToggleInstalled={(t) => setSelectedInstalled((p) => p.includes(t) ? p.filter((x) => x !== t) : [...p, t])}
          onToggleMissing={(t) => setSelectedMissing((p) => p.includes(t) ? p.filter((x) => x !== t) : [...p, t])}
          onAddMissing={handleAddMissing}
          onRequestDeleteInstalled={() => setDeleteSkillsConfirm(true)}
          onRepairCowork={onRepairCowork}
        />
      )}

      {/* Edit Dialog */}
      {editAgent && (
        <AgentEditDialog
          agent={editAgent}
          busy={busy}
          onChange={setEditAgent}
          onClose={() => setEditAgent(null)}
          onSave={handleSaveEdit}
          pickFolder={pickFolder}
          pickFile={pickFile}
        />
      )}

      {/* Delete Confirm */}
      {deleteAgent && (
        <div onClick={() => setDeleteAgent(null)} style={{ position: "fixed", inset: 0, zIndex: 60, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.36)", padding: 20 }}>
          <div onClick={(e) => e.stopPropagation()} style={{ width: "100%", maxWidth: 420, borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14)" }}>
            <div style={{ padding: "20px 24px" }}>
              <h3 style={{ fontSize: 15, fontWeight: 600, color: "var(--text)", marginBottom: 8 }}>确认删除</h3>
              <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>是否确定删除「{deleteAgent.name}」？此操作不可撤销。</p>
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "14px 24px", borderTop: "1px solid var(--border)" }}>
              <button className="btn btn-secondary" onClick={() => setDeleteAgent(null)} disabled={busy} type="button">取消</button>
              <button className="btn btn-danger" onClick={() => { onDelete(deleteAgent.id); setDeleteAgent(null); }} disabled={busy} type="button">确认删除</button>
            </div>
          </div>
        </div>
      )}

      {previewAgent && deleteSkillsConfirm && (
        <div onClick={() => setDeleteSkillsConfirm(false)} style={{ position: "fixed", inset: 0, zIndex: 70, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.36)", padding: 20 }}>
          <div onClick={(e) => e.stopPropagation()} style={{ width: "100%", maxWidth: 460, borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14)" }}>
            <div style={{ padding: "20px 24px" }}>
              <h3 style={{ fontSize: 15, fontWeight: 600, color: "var(--text)", marginBottom: 8 }}>删除已安装 Skills</h3>
              <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>
                确定要从「{previewAgent.name}」删除已选的 {selectedInstalled.length} 个 Skills 吗？其他 Agent 中的同名 Skill 不会被删除。
              </p>
            </div>
            <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "14px 24px", borderTop: "1px solid var(--border)" }}>
              <button className="btn btn-secondary" onClick={() => setDeleteSkillsConfirm(false)} disabled={busy} type="button">取消</button>
              <button className="btn btn-danger" onClick={handleDeleteInstalled} disabled={busy || selectedInstalled.length === 0} type="button">确认删除</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── 预览弹窗 ──────────────────────────────────────────────────────

function AgentPreviewDialog({ agent, installedSkills, missingSkills, selectedMissing, selectedInstalled, busy, onClose, onEdit, onDelete, onToggleInstalled, onToggleMissing, onAddMissing, onRequestDeleteInstalled, onRepairCowork }: {
  agent: AgentProfile; installedSkills: GroupedSkill[]; missingSkills: GroupedSkill[];
  selectedMissing: string[]; selectedInstalled: string[]; busy: boolean;
  onClose: () => void; onEdit: () => void; onDelete: () => void;
  onToggleInstalled: (title: string) => void;
  onToggleMissing: (title: string) => void; onAddMissing: () => void;
  onRequestDeleteInstalled: () => void;
  onRepairCowork?: (agentId: string) => Promise<unknown>;
}) {
  const adapterConfig = (agent.adapterConfig as Record<string, unknown>) ?? {};
  const mcpPath = adapterConfig.mcpConfigPath as string | undefined;
  const coworkPluginRoot = adapterConfig.pluginRoot as string | undefined;
  const coworkManifestPath = adapterConfig.manifestPath as string | undefined;
  const unregisteredSkills = installedSkills.filter((skill) => {
    const copy = skill.copies.find((item) => item.agentId === agent.id);
    return copy && copy.isRegistered === false;
  });

  return (
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ maxHeight: "88vh", width: "100%", maxWidth: 720, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" /><circle cx="12" cy="7" r="4" /></svg>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>{agent.name}</h2>
              <span className="badge" style={{ fontSize: 10 }}>{agentTypeLabel(agent.type)}</span>
            </div>
          </div>
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px 24px" }}>
          {/* 路径信息 */}
          <div style={{ display: "flex", flexDirection: "column", gap: 10, marginBottom: 20 }}>
            <div className="detail-item">
              <span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 80 }}>Skills 目录</span>
              <code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{agent.skillsPath}</code>
            </div>
            {mcpPath && (
              <div className="detail-item">
                <span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 80 }}>MCP 配置</span>
                <code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{mcpPath}</code>
              </div>
            )}
            {coworkPluginRoot && (
              <div className="detail-item">
                <span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 80 }}>插件包</span>
                <code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{coworkPluginRoot}</code>
              </div>
            )}
            {coworkManifestPath && (
              <div className="detail-item">
                <span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 80 }}>Cowork 清单</span>
                <code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{coworkManifestPath}</code>
              </div>
            )}
          </div>

          {/* 概览指标 */}
          <div style={{ display: "flex", gap: 8, marginBottom: 20 }}>
            <div style={{ flex: 1, padding: "10px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", background: "var(--surface)", textAlign: "center" }}>
              <div style={{ fontSize: 18, fontWeight: 600, color: "var(--success)" }}>{installedSkills.length}</div>
              <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>已安装</div>
            </div>
            <div style={{ flex: 1, padding: "10px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", background: "var(--surface)", textAlign: "center" }}>
              <div style={{ fontSize: 18, fontWeight: 600, color: "var(--warning)" }}>{missingSkills.length}</div>
              <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>缺失</div>
            </div>
          </div>

          {/* Skills 对照 */}
          <div className="agent-skill-columns">
            <section className="agent-skill-column">
              <div className="agent-skill-column-title">已安装 Skills ({installedSkills.length})</div>
              <div className="agent-skill-list">
                {installedSkills.map((skill) => (
                  <AgentSkillCard
                    key={skill.title}
                    agentId={agent.id}
                    skill={skill}
                    kind="installed"
                    selected={selectedInstalled.includes(skill.title)}
                    onToggle={() => onToggleInstalled(skill.title)}
                  />
                ))}
                {!installedSkills.length && <div className="agent-skill-empty">暂无已安装 skills</div>}
              </div>
            </section>

            <section className="agent-skill-column">
              <div className="agent-skill-column-title">缺失 Skills ({missingSkills.length})</div>
              <div className="agent-skill-list">
                {missingSkills.map((skill) => (
                  <AgentSkillCard
                    key={skill.title}
                    agentId={agent.id}
                    skill={skill}
                    kind="missing"
                    selected={selectedMissing.includes(skill.title)}
                    onToggle={() => onToggleMissing(skill.title)}
                  />
                ))}
                {!missingSkills.length && <div className="agent-skill-empty">暂无缺失 skills</div>}
              </div>
            </section>
          </div>
        </div>

        {/* Footer */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, borderTop: "1px solid var(--border)", padding: "16px 24px" }}>
          <button className="btn btn-secondary" onClick={onDelete} disabled={busy} type="button" style={{ color: "var(--danger)" }}>
            <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
            删除
          </button>
          <div style={{ display: "flex", gap: 8 }}>
            {agent.type === "claudeCowork" && unregisteredSkills.length > 0 && onRepairCowork && (
              <button className="btn btn-secondary" onClick={() => onRepairCowork(agent.id)} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M21 12a9 9 0 0 1-9 9" /><path d="M3 12a9 9 0 0 1 9-9" /><polyline points="16 16 12 12 8 16" /><line x1="12" y1="12" x2="12" y2="21" /></svg>
                修复 Cowork 清单
              </button>
            )}
            {selectedMissing.length > 0 && (
              <button className="btn btn-primary" onClick={onAddMissing} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>
                添加 {selectedMissing.length} 个 Skills
              </button>
            )}
            {selectedInstalled.length > 0 && (
              <button className="btn btn-danger" onClick={onRequestDeleteInstalled} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
                删除 {selectedInstalled.length} 个 Skills
              </button>
            )}
            <button className="btn btn-primary" onClick={onEdit} disabled={busy} type="button">
              <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
              编辑
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function AgentSkillCard({ agentId, skill, kind, selected = false, onToggle }: {
  agentId: string;
  skill: GroupedSkill;
  kind: "installed" | "missing";
  selected?: boolean;
  onToggle?: () => void;
}) {
  const agentCopy = skill.copies.find((copy) => copy.agentId === agentId);
  const description = skillDescription(skill, agentId);
  const isMissing = kind === "missing";
  const className = [
    "agent-skill-card",
    isMissing ? "missing" : "installed",
    selected ? "selected" : "",
  ].filter(Boolean).join(" ");

  return (
    <div
      className={className}
      onClick={onToggle}
      role={onToggle ? "button" : undefined}
      tabIndex={onToggle ? 0 : undefined}
      title={description}
    >
      <div className="agent-skill-card-main">
        <div className="agent-skill-card-name">{skill.title}</div>
        <div className="agent-skill-card-desc">{description}</div>
      </div>
      <div className="agent-skill-card-badges">
        {!isMissing && agentCopy?.isRegistered === false && <span className="badge badge-warning">未注册</span>}
        {isMissing ? (
          <span className={selected ? "badge badge-syncable" : "badge badge-warning"}>{selected ? "已选" : "缺失"}</span>
        ) : selected ? (
          <span className="badge badge-warning">待删除</span>
        ) : (
          <span className="badge badge-version">{skill.bestCopy.version ? `v${skill.bestCopy.version}` : "-"}</span>
        )}
      </div>
    </div>
  );
}

function skillDescription(skill: GroupedSkill, agentId: string): string {
  const agentCopyDescription = skill.copies
    .find((copy) => copy.agentId === agentId)
    ?.description
    ?.trim();
  const description = agentCopyDescription || skill.description?.trim() || skill.bestCopy.description?.trim();
  return description || "暂无描述";
}

// ── 编辑弹窗 ──────────────────────────────────────────────────────

function AgentEditDialog({ agent, busy, onChange, onClose, onSave, pickFolder, pickFile }: {
  agent: AgentProfile; busy: boolean;
  onChange: (agent: AgentProfile) => void; onClose: () => void; onSave: () => void;
  pickFolder: () => Promise<string | null>;
  pickFile: (filters?: Array<{ name: string; extensions: string[] }>) => Promise<string | null>;
}) {
  const showMcpPath = isMcpAgent(agent.type);

  return (
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ maxHeight: "88vh", width: "100%", maxWidth: 540, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
          </div>
          <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>编辑 Agent</h2>
          <div style={{ flex: 1 }} />
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px 24px" }}>
          <div className="input-group">
            <label className="input-label">类型</label>
            <select className="input" value={agent.type} onChange={(e) => onChange({ ...agent, type: e.target.value as AgentType, adapterConfig: isMcpAgent(e.target.value as AgentType) ? { mcpConfigPath: "" } : {} })}>
              {agentTypeOptions.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
            </select>
          </div>
          <div className="input-group">
            <label className="input-label">名称</label>
            <input className="input" value={agent.name} onChange={(e) => onChange({ ...agent, name: e.target.value })} placeholder="Agent 名称" />
          </div>
          <div className="input-group">
            <label className="input-label">Skills 安装目录</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input className="input" value={agent.skillsPath} onChange={(e) => onChange({ ...agent, skillsPath: e.target.value })} placeholder="C:\Users\you\.agent\skills" style={{ flex: 1 }} />
              <button className="btn btn-secondary" onClick={async () => { const p = await pickFolder(); if (p) onChange({ ...agent, skillsPath: p }); }} disabled={busy} type="button">浏览</button>
            </div>
          </div>
          {showMcpPath && (
            <div className="input-group">
              <label className="input-label">MCP 配置文件路径（可选）</label>
              <div style={{ display: "flex", gap: 8 }}>
                <input className="input" value={(agent.adapterConfig as Record<string, unknown>)?.mcpConfigPath as string ?? ""} onChange={(e) => onChange({ ...agent, adapterConfig: { ...agent.adapterConfig, mcpConfigPath: e.target.value } })} placeholder={mcpPlaceholder(agent.type)} style={{ flex: 1 }} />
                <button className="btn btn-secondary" onClick={async () => { const p = await pickFile(mcpFileFilter(agent.type)); if (p) onChange({ ...agent, adapterConfig: { ...agent.adapterConfig, mcpConfigPath: p } }); }} disabled={busy} type="button">浏览</button>
              </div>
              <p style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>{mcpHint(agent.type)}</p>
            </div>
          )}
        </div>

        {/* Footer */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, borderTop: "1px solid var(--border)", padding: "16px 24px" }}>
          <button className="btn btn-secondary" onClick={onClose} disabled={busy} type="button">取消</button>
          <button className="btn btn-primary" onClick={onSave} disabled={busy || !agent.name.trim() || !agent.skillsPath.trim()} type="button">保存</button>
        </div>
      </div>
    </div>
  );
}

// ── 添加 Agent 面板 (右侧常驻) ──────────────────────────────────────

function AddAgentPanel({ customAgent, busy, onCustomChange, onSaveCustom, pickFolder, pickFile }: {
  customAgent: AgentProfile; busy: boolean;
  onCustomChange: (agent: AgentProfile) => void; onSaveCustom: () => void;
  pickFolder: () => Promise<string | null>;
  pickFile: (filters?: Array<{ name: string; extensions: string[] }>) => Promise<string | null>;
}) {
  const showMcpPath = isMcpAgent(customAgent.type);

  return (
    <div className="card">
      <div className="card-header">
        <div className="card-title">添加 Agent</div>
        <div className="card-desc">添加一个本地目录作为 Agent 配置</div>
      </div>
      <div className="card-body">
        <div className="input-group">
          <label className="input-label">类型</label>
          <select className="input" value={customAgent.type} onChange={(e) => { const t = e.target.value as AgentType; onCustomChange({ ...customAgent, type: t, adapterConfig: isMcpAgent(t) ? { mcpConfigPath: "" } : {} }); }}>
            {agentTypeOptions.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
          </select>
        </div>
        <div className="input-group">
          <label className="input-label">名称</label>
          <input className="input" value={customAgent.name} onChange={(e) => onCustomChange({ ...customAgent, name: e.target.value })} placeholder={agentPlaceholder(customAgent.type)} />
        </div>
        <div className="input-group">
          <label className="input-label">Skills 安装目录</label>
          <div style={{ display: "flex", gap: 8 }}>
            <input className="input" value={customAgent.skillsPath} onChange={(e) => onCustomChange({ ...customAgent, skillsPath: e.target.value })} placeholder={skillsPlaceholder(customAgent.type)} style={{ flex: 1 }} />
            <button className="btn btn-secondary" onClick={async () => { const p = await pickFolder(); if (p) onCustomChange({ ...customAgent, skillsPath: p }); }} disabled={busy} type="button">浏览</button>
          </div>
        </div>
        {showMcpPath && (
          <div className="input-group">
            <label className="input-label">MCP 配置文件路径（可选）</label>
            <div style={{ display: "flex", gap: 8 }}>
              <input className="input" value={(customAgent.adapterConfig as Record<string, unknown>)?.mcpConfigPath as string ?? ""} onChange={(e) => onCustomChange({ ...customAgent, adapterConfig: { ...customAgent.adapterConfig, mcpConfigPath: e.target.value } })} placeholder={mcpPlaceholder(customAgent.type)} style={{ flex: 1 }} />
              <button className="btn btn-secondary" onClick={async () => { const p = await pickFile(mcpFileFilter(customAgent.type)); if (p) onCustomChange({ ...customAgent, adapterConfig: { ...customAgent.adapterConfig, mcpConfigPath: p } }); }} disabled={busy} type="button">浏览</button>
            </div>
            <p style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>{mcpHint(customAgent.type)}</p>
          </div>
        )}
        <button className="btn btn-primary" onClick={() => onSaveCustom()} disabled={busy} type="button" style={{ marginTop: 4 }}>
          <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>
          添加 Agent
        </button>
      </div>
    </div>
  );
}

// ── 辅助函数 ──────────────────────────────────────────────────────

function agentTypeLabel(type: AgentType): string {
  const map: Record<AgentType, string> = { codex: "Codex", claude: "Claude", claudeCode: "Claude Code", claudeCowork: "Claude Desktop Cowork", cursor: "Cursor", trae: "Trae", custom: "自定义", cherryStudio: "Cherry Studio", opencode: "OpenCode" };
  return map[type] ?? type;
}

function agentPlaceholder(type: AgentType): string {
  const map: Partial<Record<AgentType, string>> = { opencode: "OpenCode", codex: "Codex", claudeCode: "Claude Code", claudeCowork: "Claude Desktop Cowork", cursor: "Cursor", trae: "Trae" };
  return map[type] ?? "例如 My Agent";
}

function skillsPlaceholder(type: AgentType): string {
  const map: Partial<Record<AgentType, string>> = { opencode: "~/.opencode/skills", codex: "~/.codex/skills", claudeCode: "~/.claude/skills", claudeCowork: "%LOCALAPPDATA%\\Claude-3p\\...\\skills", cursor: "~/.cursor/skills", trae: "~/.trae/skills" };
  return map[type] ?? "C:\\Users\\you\\.agent\\skills";
}

function mcpPlaceholder(type: AgentType): string {
  const map: Partial<Record<AgentType, string>> = { opencode: "~/.opencode.json", codex: "~/.codex/config.toml", claudeCode: "~/.claude.json", trae: "~/.trae/mcp.json" };
  return map[type] ?? "MCP 配置文件路径";
}

function mcpHint(type: AgentType): string {
  const map: Partial<Record<AgentType, string>> = { opencode: "留空使用默认 ~/.opencode.json", codex: "留空使用默认 ~/.codex/config.toml", claudeCode: "留空使用默认 ~/.claude.json", trae: "留空使用默认 ~/.trae/mcp.json" };
  return map[type] ?? "留空则使用默认路径";
}

function mcpFileFilter(type: AgentType): Array<{ name: string; extensions: string[] }> | undefined {
  const map: Partial<Record<AgentType, Array<{ name: string; extensions: string[] }>>> = { opencode: [{ name: "JSON", extensions: ["json"] }], codex: [{ name: "TOML", extensions: ["toml"] }], claudeCode: [{ name: "JSON", extensions: ["json"] }], trae: [{ name: "JSON", extensions: ["json"] }] };
  return map[type];
}
