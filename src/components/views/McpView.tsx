import { useEffect, useMemo, useRef, useState } from "react";
import type {
  AgentProfile,
  ConflictPolicy,
  GroupedMcpServer,
  McpServerConfig,
  McpTransport,
} from "../../types";

interface McpViewProps {
  servers: GroupedMcpServer[];
  agents: AgentProfile[];
  busy: boolean;
  noFullCoverageMcpTitles: Set<string>;
  onAdd: (agentIds: string[], config: McpServerConfig, conflictPolicy: ConflictPolicy) => Promise<unknown>;
  onUpdate: (agentId: string, originalName: string, config: McpServerConfig) => Promise<unknown>;
  onRemove: (agentId: string, name: string) => Promise<unknown>;
  onToggle: (agentId: string, name: string, disabled: boolean) => Promise<unknown>;
  onRefresh: () => Promise<void>;
  onSyncToAgents: (serverName: string, sourceAgentId: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<unknown>;
  onRemoveFromAgents: (serverName: string, agentIds: string[]) => Promise<unknown>;
  onToggleNoFullCoverage: (title: string) => Promise<void>;
}

export function McpView({ servers, agents, busy, noFullCoverageMcpTitles, onAdd, onUpdate, onRemove, onToggle, onRefresh, onSyncToAgents, onRemoveFromAgents, onToggleNoFullCoverage }: McpViewProps) {
  const [showForm, setShowForm] = useState(false);
  const [editingServer, setEditingServer] = useState<GroupedMcpServer | null>(null);
  const [selectedServer, setSelectedServer] = useState<GroupedMcpServer | null>(null);
  const [selectedAgentIds, setSelectedAgentIds] = useState<string[]>([]);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");
  const [filter, setFilter] = useState<"all" | "covered" | "partial" | "needed">("all");
  const [discardConfirm, setDiscardConfirm] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [formDiscardConfirm, setFormDiscardConfirm] = useState(false);

  const mcpAgents = agents.filter((a) => {
    if (a.type === "codex" || a.type === "claudeCode" || a.type === "opencode" || a.type === "trae") return true;
    if (a.type === "custom" && a.adapterConfig?.mcpFormat) return true;
    return false;
  });

  const coveredCount = useMemo(() => {
    return servers.filter((s) => s.agentIds.length === mcpAgents.length).length;
  }, [servers, mcpAgents.length]);

  const partialCount = useMemo(() => {
    return servers.filter((s) => s.agentIds.length < mcpAgents.length && noFullCoverageMcpTitles.has(s.name)).length;
  }, [servers, mcpAgents.length, noFullCoverageMcpTitles]);

  const neededCount = useMemo(() => {
    return servers.filter((s) => s.agentIds.length < mcpAgents.length && !noFullCoverageMcpTitles.has(s.name)).length;
  }, [servers, mcpAgents.length, noFullCoverageMcpTitles]);

  const displayedServers = useMemo(() => {
    if (filter === "covered") return servers.filter((s) => s.agentIds.length === mcpAgents.length);
    if (filter === "partial") return servers.filter((s) => s.agentIds.length < mcpAgents.length && noFullCoverageMcpTitles.has(s.name));
    if (filter === "needed") return servers.filter((s) => s.agentIds.length < mcpAgents.length && !noFullCoverageMcpTitles.has(s.name));
    return servers;
  }, [servers, filter, mcpAgents.length, noFullCoverageMcpTitles]);

  function handleAdd() {
    setEditingServer(null);
    setSelectedAgentIds(mcpAgents.map((a) => a.id));
    setShowForm(true);
  }

  function handleOpenSync(server: GroupedMcpServer) {
    setSelectedServer(server);
    setSelectedAgentIds(server.agentIds);
    setConflictPolicy("backupOverwrite");
  }

  function handleEditFromDialog(server: GroupedMcpServer) {
    setSelectedServer(null);
    setEditingServer(server);
    setSelectedAgentIds(server.agentIds);
    setShowForm(true);
  }

  async function handleToggleFromDialog(agentId: string, name: string, currentlyDisabled: boolean) {
    await onToggle(agentId, name, !currentlyDisabled);
    const updated = servers.find((s) => s.name === name);
    if (updated) {
      setSelectedServer(updated);
      setSelectedAgentIds(updated.agentIds);
    }
  }

  function toggleSyncAgent(agentId: string) {
    setSelectedAgentIds((prev) =>
      prev.includes(agentId) ? prev.filter((id) => id !== agentId) : [...prev, agentId],
    );
  }

  async function executeSync() {
    if (!selectedServer) return;
    const configuredIds = selectedServer.agentIds;
    const deselectedIds = configuredIds.filter((id) => !selectedAgentIds.includes(id));
    // 从取消勾选的 Agent 删除
    if (deselectedIds.length > 0) {
      await onRemoveFromAgents(selectedServer.name, deselectedIds);
    }
    // 同步到勾选的 Agent
    if (selectedAgentIds.length > 0) {
      const sourceId = selectedServer.copies[0]?.agentId ?? configuredIds[0];
      if (sourceId) {
        await onSyncToAgents(selectedServer.name, sourceId, selectedAgentIds, conflictPolicy);
      }
    }
    setSelectedServer(null);
  }

  const hasSyncChanges = useMemo(() => {
    if (!selectedServer) return false;
    const initial = selectedServer.agentIds;
    if (selectedAgentIds.length !== initial.length || selectedAgentIds.some((id) => !initial.includes(id))) return true;
    if (conflictPolicy !== "backupOverwrite") return true;
    return false;
  }, [selectedServer, selectedAgentIds, conflictPolicy]);

  function requestClose() {
    if (hasSyncChanges) {
      setDiscardConfirm(true);
    } else {
      setSelectedServer(null);
    }
  }

  function requestDeleteAll() {
    setDeleteConfirm(true);
  }

  function handleSyncRequest() {
    if (selectedAgentIds.length === 0) {
      requestDeleteAll();
    } else {
      executeSync();
    }
  }

  function confirmDeleteAll() {
    setDeleteConfirm(false);
    // 直接从所有已配置的 Agent 删除
    if (!selectedServer) return;
    onRemoveFromAgents(selectedServer.name, selectedServer.agentIds).then(() => {
      setSelectedServer(null);
    });
  }

  function requestFormClose() {
    setFormDiscardConfirm(true);
  }

  return (
    <>
      <div className="view-header">
        <div>
          <h2 className="view-title">MCP Servers</h2>
          <p className="view-subtitle">管理 Codex、Claude Code、OpenCode 和 Trae 的 MCP server 配置</p>
        </div>
        <div className="view-header-actions">
          <button className="btn btn-secondary" onClick={onRefresh} disabled={busy}>刷新</button>
          <button className="btn btn-primary" onClick={handleAdd} disabled={busy}>添加 MCP</button>
        </div>
      </div>

      <div className="metrics">
        <div
          className="metric-card"
          onClick={() => setFilter("all")}
          style={{ cursor: filter !== "all" ? "pointer" : "default" }}
        >
          <div className="metric-value">{servers.length}</div>
          <div className="metric-label">MCP</div>
        </div>
        <div
          className="metric-card"
          onClick={() => setFilter((f) => f === "covered" ? "all" : "covered")}
          style={{ cursor: "pointer", ...(filter === "covered" ? { borderColor: "var(--success)", background: "var(--success-light)" } : {}) }}
        >
          <div className="metric-value success">{coveredCount}</div>
          <div className="metric-label">完全覆盖</div>
        </div>
        <div
          className="metric-card"
          onClick={() => setFilter((f) => f === "partial" ? "all" : "partial")}
          style={{ cursor: "pointer", ...(filter === "partial" ? { borderColor: "var(--accent)", background: "var(--accent-light)" } : {}) }}
        >
          <div className="metric-value">{partialCount}</div>
          <div className="metric-label">已部分覆盖</div>
        </div>
        <div
          className="metric-card"
          onClick={() => setFilter((f) => f === "needed" ? "all" : "needed")}
          style={{ cursor: "pointer", ...(filter === "needed" ? { borderColor: "var(--warning)", background: "var(--warning-light)" } : {}) }}
        >
          <div className="metric-value warning">{neededCount}</div>
          <div className="metric-label">需同步</div>
        </div>
      </div>

      <div className="skills-panel">
        <div className="skills-header">
          <div><div className="card-title">MCP Servers 控制台</div><div className="card-desc">点击任意 MCP server 查看详情和管理配置</div></div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}><span className="skills-header-badge">{displayedServers.length} 个可见项目</span></div>
        </div>
        <div className="skills-list">
          {displayedServers.length === 0 ? (
            <div style={{ textAlign: "center", padding: "48px 0", color: "var(--text-secondary)" }}>
              <p style={{ fontSize: 14, fontWeight: 500 }}>{servers.length === 0 ? "暂无 MCP server 配置" : "当前筛选无匹配项"}</p>
              <p style={{ fontSize: 12, color: "var(--text-tertiary)", marginTop: 4 }}>{servers.length === 0 ? "点击\"添加 MCP\"开始配置" : "切换筛选条件查看更多"}</p>
            </div>
          ) : (
            displayedServers.map((server) => (
              <McpServerItem key={server.name} server={server} mcpAgents={mcpAgents} noFullCoverageMcpTitles={noFullCoverageMcpTitles} onClick={() => handleOpenSync(server)} />
            ))
          )}
        </div>
      </div>

      {selectedServer && (
        <McpSyncDialog
          server={selectedServer}
          mcpAgents={mcpAgents}
          busy={busy}
          selectedAgentIds={selectedAgentIds}
          conflictPolicy={conflictPolicy}
          isNoFullCoverage={noFullCoverageMcpTitles.has(selectedServer.name)}
          onClose={requestClose}
          onEdit={() => handleEditFromDialog(selectedServer)}
          onToggleAgent={toggleSyncAgent}
          onPolicy={setConflictPolicy}
          onSync={handleSyncRequest}
          onToggle={handleToggleFromDialog}
          onToggleNoFullCoverage={() => onToggleNoFullCoverage(selectedServer.name)}
        />
      )}

      {showForm && (
        <McpFormDialog
          key={editingServer?.name ?? "new"}
          server={editingServer}
          agents={mcpAgents}
          selectedAgentIds={selectedAgentIds}
          onClose={requestFormClose}
          onSubmit={async (agentIds, config, policy) => {
            if (editingServer) {
              for (const agentId of agentIds) {
                await onUpdate(agentId, editingServer.name, config);
              }
            } else {
              await onAdd(agentIds, config, policy);
            }
            setShowForm(false);
          }}
        />
      )}

      {discardConfirm && (
        <ConfirmDialog
          title="放弃更改"
          message="当前有未保存的更改，确定要放弃吗？"
          confirmLabel="放弃"
          busy={busy}
          onClose={() => setDiscardConfirm(false)}
          onConfirm={() => { setDiscardConfirm(false); setSelectedServer(null); }}
        />
      )}

      {deleteConfirm && (
        <ConfirmDialog
          title="全部删除"
          message={`确定要从所有 ${selectedServer?.agentIds.length ?? 0} 个 Agent 中删除 "${selectedServer?.name}" 吗？`}
          confirmLabel="全部删除"
          busy={busy}
          onClose={() => setDeleteConfirm(false)}
          onConfirm={confirmDeleteAll}
        />
      )}

      {formDiscardConfirm && (
        <ConfirmDialog
          title="放弃更改"
          message="当前有未保存的更改，确定要放弃吗？"
          confirmLabel="放弃"
          busy={busy}
          onClose={() => setFormDiscardConfirm(false)}
          onConfirm={() => { setFormDiscardConfirm(false); setShowForm(false); }}
        />
      )}

    </>
  );
}

// ── MCP Server 列表项 ─────────────────────────────────────────────────

function McpServerItem({ server, mcpAgents, noFullCoverageMcpTitles, onClick }: { server: GroupedMcpServer; mcpAgents: AgentProfile[]; noFullCoverageMcpTitles: Set<string>; onClick: () => void }) {
  const transportBadge = (transport: McpTransport) => {
    const cls: Record<McpTransport, string> = { stdio: "badge-stdio", http: "badge-http", sse: "badge-sse" };
    const lbl: Record<McpTransport, string> = { stdio: "STDIO", http: "HTTP", sse: "SSE" };
    return <span className={`badge ${cls[transport]}`}>{lbl[transport]}</span>;
  };

  return (
    <div className="skill-item" onClick={onClick} role="button" tabIndex={0}>
      <div className="skill-icon">
        <svg className="icon" viewBox="0 0 24 24"><circle cx="12" cy="12" r="3" /><path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M16.95 16.95l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M16.95 7.05l2.83-2.83" /></svg>
      </div>
      <div className="skill-info">
        <div className="skill-name">{server.name}{server.copies[0] && transportBadge(server.copies[0].config.transport)}</div>
        <div className="skill-meta">{server.copies[0]?.config.command && `命令: ${server.copies[0].config.command}`}{server.copies[0]?.config.url && `URL: ${server.copies[0].config.url}`}</div>
        {server.copies[0]?.config.args && server.copies[0].config.args.length > 0 && <div className="skill-desc">参数: {server.copies[0].config.args.join(" ")}</div>}
      </div>
      <span className={`badge ${server.agentIds.length === mcpAgents.length ? "badge-success" : "badge-warning"}`}>{server.agentIds.length}/{mcpAgents.length} Agent</span>
      <span className={`badge ${server.agentIds.length === mcpAgents.length ? "badge-synced" : noFullCoverageMcpTitles.has(server.name) ? "badge-muted" : "badge-syncable"}`}>
        {server.agentIds.length === mcpAgents.length ? "已覆盖" : noFullCoverageMcpTitles.has(server.name) ? "已部分覆盖" : "需同步"}
      </span>
    </div>
  );
}

// ── MCP 同步弹窗 ─────────────────────────────────────────────────────

const mcpPolicyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新目标配置" },
  { value: "skip", label: "跳过冲突", helper: "目标已存在时不做修改" },
];

function McpSyncDialog({ server, mcpAgents, busy, selectedAgentIds, conflictPolicy, isNoFullCoverage, onClose, onEdit, onToggleAgent, onPolicy, onSync, onToggle, onToggleNoFullCoverage }: {
  server: GroupedMcpServer; mcpAgents: AgentProfile[]; busy: boolean;
  selectedAgentIds: string[]; conflictPolicy: ConflictPolicy; isNoFullCoverage: boolean;
  onClose: () => void; onEdit: () => void;
  onToggleAgent: (agentId: string) => void;
  onPolicy: (policy: ConflictPolicy) => void;
  onSync: () => void;
  onToggle: (agentId: string, name: string, disabled: boolean) => Promise<void>;
  onToggleNoFullCoverage: () => void;
}) {
  const config = server.copies[0]?.config;
  const rawConfig = server.copies[0]?.rawConfig;

  return (
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ maxHeight: "88vh", width: "100%", maxWidth: 960, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><circle cx="12" cy="12" r="3" /><path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M16.95 16.95l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M16.95 7.05l2.83-2.83" /></svg>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>{server.name}</h2>
            <p style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>{config?.transport.toUpperCase()} · {server.copies.length} 个配置</p>
          </div>
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        {/* Body - Two Column Layout */}
        <div style={{ flex: 1, overflow: "hidden", display: "grid", gridTemplateColumns: "1.2fr 1fr" }}>
          {/* Left Column - Config Details */}
          <div style={{ overflow: "auto", padding: "20px 24px", borderRight: "1px solid var(--border)" }}>
            <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 12 }}>配置详情</p>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <div className="detail-item"><span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 60 }}>类型</span><span className={`badge badge-${config?.transport}`}>{config?.transport.toUpperCase()}</span></div>
              {config?.command && <div className="detail-item"><span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 60 }}>命令</span><code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{config.command}</code></div>}
              {config?.args && config.args.length > 0 && <div className="detail-item"><span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 60 }}>参数</span><code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{config.args.join(" ")}</code></div>}
              {config?.url && <div className="detail-item"><span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 60 }}>URL</span><code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{config.url}</code></div>}
              {config?.env && Object.keys(config.env).length > 0 && (
                <div style={{ padding: "10px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", background: "var(--surface)" }}>
                  <p style={{ fontSize: 12, fontWeight: 500, color: "var(--text-secondary)", marginBottom: 8 }}>环境变量</p>
                  <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                    {Object.entries(config.env).map(([key, value]) => <div key={key} style={{ fontSize: 12, fontFamily: "var(--font-mono)" }}><span style={{ color: "var(--accent)" }}>{key}</span>=<span style={{ color: "var(--text-tertiary)" }}>{maskValue(value)}</span></div>)}
                  </div>
                </div>
              )}
              {config?.headers && Object.keys(config.headers).length > 0 && (
                <div style={{ padding: "10px 12px", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", background: "var(--surface)" }}>
                  <p style={{ fontSize: 12, fontWeight: 500, color: "var(--text-secondary)", marginBottom: 8 }}>Headers</p>
                  <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                    {Object.entries(config.headers).map(([key, value]) => <div key={key} style={{ fontSize: 12, fontFamily: "var(--font-mono)" }}><span style={{ color: "var(--accent)" }}>{key}</span>: <span style={{ color: "var(--text-tertiary)" }}>{maskValue(value)}</span></div>)}
                  </div>
                </div>
              )}
            </div>

            {/* Raw Config */}
            {rawConfig && (
              <div style={{ marginTop: 16 }}>
                <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>原始配置</p>
                <pre style={{ fontFamily: "var(--font-mono)", fontSize: 12, lineHeight: 1.6, padding: "12px 14px", background: "var(--surface)", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", whiteSpace: "pre-wrap", wordBreak: "break-all" }}>{rawConfig}</pre>
              </div>
            )}
          </div>

          {/* Right Column - Agent Sync */}
          <div style={{ overflow: "auto", padding: "20px 24px", display: "flex", flexDirection: "column", gap: 20 }}>
            {/* Agent List with Checkboxes */}
            <div>
              <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>目标 Agent</p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {mcpAgents.map((agent) => {
                  const checked = selectedAgentIds.includes(agent.id);
                  const copy = server.copies.find((c) => c.agentId === agent.id);
                  const isDisabled = server.disabledAgentIds?.includes(agent.id);
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
                        <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)" }}>{copy ? (isDisabled ? "已禁用" : "已配置") : "未配置"}</span>
                      </span>
                      {copy && (
                        <button
                          className="btn btn-sm btn-secondary"
                          onClick={(e) => { e.stopPropagation(); onToggle(agent.id, server.name, isDisabled ?? false); }}
                          disabled={busy}
                          type="button"
                        >
                          {isDisabled ? "启用" : "禁用"}
                        </button>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Conflict Policy */}
            <div>
              <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>冲突策略</p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {mcpPolicyOptions.map((option) => (
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
          <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>已选择 {selectedAgentIds.length} 个 Agent</p>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              className="btn btn-secondary"
              onClick={onToggleNoFullCoverage}
              disabled={busy}
              type="button"
              style={isNoFullCoverage ? { borderColor: "var(--accent)", color: "var(--accent)" } : {}}
            >
              <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" /></svg>
              {isNoFullCoverage ? "取消无需全覆盖" : "无需全覆盖"}
            </button>
            <button className="btn btn-secondary" onClick={onEdit} disabled={busy} type="button">编辑配置</button>
            {selectedAgentIds.length === 0 ? (
              <button className="btn btn-danger" onClick={onSync} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
                全部删除
              </button>
            ) : (
              <button className="btn btn-primary" onClick={onSync} disabled={busy} type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
                {selectedAgentIds.length < server.agentIds.length ? "同步并清理" : "同步"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── 辅助函数 ──────────────────────────────────────────────────────────

function maskValue(value: string): string {
  if (value.length <= 8) return "***";
  return value.slice(0, 4) + "..." + value.slice(-4);
}

function parseEnvText(text: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf("=");
    if (idx > 0) result[trimmed.slice(0, idx).trim()] = trimmed.slice(idx + 1).trim();
  }
  return result;
}

function parseHeaderText(text: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf(":");
    if (idx > 0) result[trimmed.slice(0, idx).trim()] = trimmed.slice(idx + 1).trim();
  }
  return result;
}

function envToText(env: Record<string, string> | undefined): string {
  if (!env) return "";
  return Object.entries(env).map(([k, v]) => `${k}=${v}`).join("\n");
}

function headersToText(headers: Record<string, string> | undefined): string {
  if (!headers) return "";
  return Object.entries(headers).map(([k, v]) => `${k}: ${v}`).join("\n");
}

function buildFullJson(name: string, config: McpServerConfig): string {
  const serverObj: Record<string, unknown> = {};
  if (config.transport === "stdio") {
    if (config.command) serverObj.command = config.command;
    if (config.args && config.args.length > 0) serverObj.args = config.args;
    if (config.env && Object.keys(config.env).length > 0) serverObj.env = config.env;
  } else {
    if (config.url) serverObj.url = config.url;
    if (config.transport === "sse") serverObj.transport = "sse";
    if (config.headers && Object.keys(config.headers).length > 0) serverObj.headers = config.headers;
  }
  return JSON.stringify({ [name]: serverObj }, null, 2);
}

// ── MCP 表单对话框 ────────────────────────────────────────────────────

function McpFormDialog({ server, agents, selectedAgentIds, onClose, onSubmit }: {
  server: GroupedMcpServer | null; agents: AgentProfile[]; selectedAgentIds: string[];
  onClose: () => void;
  onSubmit: (agentIds: string[], config: McpServerConfig, conflictPolicy: ConflictPolicy) => Promise<void>;
}) {
  const isEdit = server !== null;
  const existingConfig = server?.copies[0]?.config;

  const [name, setName] = useState(server?.name ?? "");
  const [transport, setTransport] = useState<McpTransport>(existingConfig?.transport ?? "stdio");
  const [command, setCommand] = useState(existingConfig?.command ?? "");
  const [args, setArgs] = useState(existingConfig?.args?.join(" ") ?? "");
  const [env, setEnv] = useState(envToText(existingConfig?.env));
  const [url, setUrl] = useState(existingConfig?.url ?? "");
  const [headers, setHeaders] = useState(headersToText(existingConfig?.headers));
  const [targetAgentIds, setTargetAgentIds] = useState<string[]>(selectedAgentIds);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");
  const [submitting, setSubmitting] = useState(false);
  const [inputMode, setInputMode] = useState<"form" | "json">("form");
  const [jsonInput, setJsonInput] = useState("");
  const jsonRef = useRef<HTMLTextAreaElement>(null);
  const envRef = useRef<HTMLTextAreaElement>(null);
  const headersRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (inputMode === "json" && jsonRef.current) {
      const el = jsonRef.current;
      el.style.height = "auto";
      el.style.height = el.scrollHeight + "px";
    }
  }, [jsonInput, inputMode]);

  useEffect(() => {
    [envRef.current, headersRef.current].forEach((el) => {
      if (el) {
        el.style.height = "auto";
        el.style.height = el.scrollHeight + "px";
      }
    });
  }, [env, headers, inputMode]);

  function switchToJson() {
    const fullJson = buildFullJson(name || "unnamed", {
      name: name || "unnamed",
      transport,
      command: command || undefined,
      args: args.trim() ? args.trim().split(/\s+/).filter(Boolean) : [],
      env: parseEnvText(env),
      url: url || undefined,
      headers: parseHeaderText(headers),
    });
    setJsonInput(fullJson);
    setInputMode("json");
  }

  function unwrapMcpServers(parsed: Record<string, unknown>): { serverName: string; obj: Record<string, unknown> } {
    // 处理 {"mcpServers": {"name": {...}}} 格式
    const firstKey = Object.keys(parsed)[0];
    if (firstKey === "mcpServers" && typeof parsed[firstKey] === "object" && parsed[firstKey] !== null && !Array.isArray(parsed[firstKey])) {
      const inner = parsed[firstKey] as Record<string, unknown>;
      const innerKey = Object.keys(inner)[0];
      if (innerKey && typeof inner[innerKey] === "object" && inner[innerKey] !== null) {
        return { serverName: innerKey, obj: inner[innerKey] as Record<string, unknown> };
      }
    }
    // 处理 {"name": {...}} 格式
    return { serverName: firstKey ?? "unnamed", obj: firstKey ? parsed[firstKey] as Record<string, unknown> : parsed };
  }

  function switchToForm() {
    try {
      const parsed = JSON.parse(jsonInput);
      const { serverName, obj } = unwrapMcpServers(parsed);
      if (!name.trim()) setName(serverName);
      const cmd = obj.command as string | undefined;
      const urlStr = obj.url as string | undefined;
      const argsArr = obj.args as string[] | undefined;
      const envObj = obj.env as Record<string, string> | undefined;
      const headersObj = obj.headers as Record<string, string> | undefined;
      const transportStr = obj.transport as string | undefined;
      if (cmd) {
        setTransport("stdio");
        setCommand(cmd ?? "");
        setArgs((argsArr ?? []).join(" "));
        setEnv(envObj ? Object.entries(envObj).map(([k, v]) => `${k}=${v}`).join("\n") : "");
      } else if (urlStr) {
        setTransport(transportStr === "sse" ? "sse" : "http");
        setUrl(urlStr ?? "");
        setHeaders(headersObj ? Object.entries(headersObj).map(([k, v]) => `${k}: ${v}`).join("\n") : "");
      }
    } catch { /* ignore */ }
    setInputMode("form");
  }

  async function handleSubmit() {
    if (!name.trim()) return;
    setSubmitting(true);
    try {
      let config: McpServerConfig;
      if (inputMode === "json") {
        const parsed = JSON.parse(jsonInput);
        const { serverName: parsedName, obj } = unwrapMcpServers(parsed);
        // 如果用户没有手动改名称，使用 JSON 中解析出的名称
        if (!name.trim() || name.trim() === "unnamed") {
          setName(parsedName);
        }
        const cmd = obj.command as string | undefined;
        const urlStr = obj.url as string | undefined;
        const argsArr = obj.args as string[] | undefined;
        const envObj = obj.env as Record<string, string> | undefined;
        const headersObj = obj.headers as Record<string, string> | undefined;
        const transportStr = obj.transport as string | undefined;
        config = {
          name: name.trim() || parsedName,
          transport: urlStr ? (transportStr === "sse" ? "sse" : "http") : "stdio",
          command: cmd,
          args: argsArr ?? [],
          env: envObj ?? {},
          url: urlStr,
          headers: headersObj ?? {},
        };
      } else {
        config = {
          name: name.trim(),
          transport,
          ...(transport === "stdio"
            ? { command: command.trim() || undefined, args: args.trim() ? args.trim().split(/\s+/).filter(Boolean) : [], env: parseEnvText(env) }
            : { url: url.trim() || undefined, headers: parseHeaderText(headers) }),
        };
      }
      await onSubmit(targetAgentIds, config, conflictPolicy);
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ maxHeight: "88vh", width: "100%", maxWidth: 640, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><circle cx="12" cy="12" r="3" /><path d="M12 1v4M12 19v4M4.22 4.22l2.83 2.83M16.95 16.95l2.83 2.83M1 12h4M19 12h4M4.22 19.78l2.83-2.83M16.95 7.05l2.83-2.83" /></svg>
          </div>
          <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>{isEdit ? "编辑 MCP Server" : "添加 MCP Server"}</h2>
          <div style={{ flex: 1 }} />
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        {/* Body */}
        <div style={{ flex: 1, overflow: "auto", padding: "20px 24px" }}>
          <div className="input-group">
            <label className="input-label">Server 名称</label>
            <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="my-mcp-server" />
          </div>

          <div className="input-group">
            <label className="input-label">输入模式</label>
            <div style={{ display: "flex", gap: 8 }}>
              <button className={`btn btn-sm ${inputMode === "form" ? "btn-primary" : "btn-secondary"}`} onClick={switchToForm} type="button">表单</button>
              <button className={`btn btn-sm ${inputMode === "json" ? "btn-primary" : "btn-secondary"}`} onClick={switchToJson} type="button">JSON</button>
            </div>
          </div>

          {inputMode === "form" ? (
            <>
              <div className="input-group">
                <label className="input-label">Transport 类型</label>
                <div style={{ display: "flex", gap: 8 }}>
                  {(["stdio", "http", "sse"] as McpTransport[]).map((t) => (
                    <button key={t} className={`btn btn-sm ${transport === t ? "btn-primary" : "btn-secondary"}`} onClick={() => setTransport(t)} type="button">{t.toUpperCase()}</button>
                  ))}
                </div>
              </div>
              {transport === "stdio" && (
                <>
                  <div className="input-group"><label className="input-label">命令 (command)</label><input className="input" value={command} onChange={(e) => setCommand(e.target.value)} placeholder="npx" /></div>
                  <div className="input-group"><label className="input-label">参数 (args)</label><input className="input" value={args} onChange={(e) => setArgs(e.target.value)} placeholder="-y some-mcp-server" /></div>
                  <div className="input-group"><label className="input-label">环境变量 (env)</label><textarea ref={envRef} className="input" value={env} onChange={(e) => { setEnv(e.target.value); const el = e.target; el.style.height = "auto"; el.style.height = el.scrollHeight + "px"; }} placeholder={"API_KEY=value\nANOTHER_KEY=value"} style={{ fontFamily: "var(--font-mono)", fontSize: 12.5, resize: "none", overflow: "hidden", minHeight: 60 }} /></div>
                </>
              )}
              {(transport === "http" || transport === "sse") && (
                <>
                  <div className="input-group"><label className="input-label">URL</label><input className="input" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://example.com/mcp" /></div>
                  <div className="input-group"><label className="input-label">Headers</label><textarea ref={headersRef} className="input" value={headers} onChange={(e) => { setHeaders(e.target.value); const el = e.target; el.style.height = "auto"; el.style.height = el.scrollHeight + "px"; }} placeholder={"Authorization: Bearer xxx\nContent-Type: application/json"} style={{ fontFamily: "var(--font-mono)", fontSize: 12.5, resize: "none", overflow: "hidden", minHeight: 60 }} /></div>
                </>
              )}
            </>
          ) : (
            <div className="input-group">
              <label className="input-label">JSON 配置（完整结构）</label>
              <textarea
                ref={jsonRef}
                className="input"
                value={jsonInput}
                onChange={(e) => setJsonInput(e.target.value)}
                placeholder={'{\n  "mcpServers": {\n    "server-name": {\n      "command": "npx",\n      "args": ["-y", "some-mcp-server"],\n      "env": { "API_KEY": "value" }\n    }\n  }\n}'}
                style={{ fontFamily: "var(--font-mono)", fontSize: 12.5, lineHeight: 1.6, resize: "none", overflow: "hidden", minHeight: 200 }}
              />
              <p style={{ fontSize: 11, color: "var(--text-tertiary)", marginTop: 4 }}>粘贴 MCP server JSON 配置，支持 mcpServers 包裹格式或直接 server 配置</p>
            </div>
          )}

          {!isEdit && (
            <div className="input-group">
              <label className="input-label">目标 Agent</label>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {agents.map((agent) => {
                  const checked = targetAgentIds.includes(agent.id);
                  return (
                    <button key={agent.id} className={`agent-item${checked ? " selected" : ""}`} onClick={() => setTargetAgentIds(checked ? targetAgentIds.filter((id) => id !== agent.id) : [...targetAgentIds, agent.id])} type="button" style={checked ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : undefined}>
                      <span style={{ width: 20, height: 20, flexShrink: 0, borderRadius: 4, border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`, background: checked ? "var(--accent)" : "transparent", display: "flex", alignItems: "center", justifyContent: "center" }}>
                        {checked && <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="3"><polyline points="20 6 9 17 4 12" /></svg>}
                      </span>
                      <span style={{ flex: 1, minWidth: 0 }}><span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{agent.name}</span></span>
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {!isEdit && (
            <div className="input-group">
              <label className="input-label">冲突策略</label>
              <select className="input" value={conflictPolicy} onChange={(e) => setConflictPolicy(e.target.value as ConflictPolicy)}>
                <option value="backupOverwrite">备份覆盖</option>
                <option value="skip">跳过</option>
                <option value="prompt">提示</option>
              </select>
            </div>
          )}
        </div>

        {/* Footer */}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, borderTop: "1px solid var(--border)", background: "var(--surface-raised)", padding: "16px 24px" }}>
          <button className="btn btn-primary" onClick={handleSubmit} disabled={submitting || !name.trim()}>{submitting ? "提交中..." : isEdit ? "更新" : "添加"}</button>
        </div>
      </div>
    </div>
  );
}

// ── 确认对话框 ──────────────────────────────────────────────────────

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
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 60, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.36)", padding: 20 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ width: "100%", maxWidth: 420, borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14)" }}>
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
