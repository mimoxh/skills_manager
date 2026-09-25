import { useState } from "react";
import { api } from "../../api";
import { policyOptions } from "../../lib/policyOptions";
import type { AgentProfile, ConflictPolicy, RemoteSourceInspection } from "../../types";
import { Dialog } from "../ui/Dialog";

interface InstallUrlDialogProps {
  agents: AgentProfile[];
  busy: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

export function InstallUrlDialog({
  agents,
  busy,
  onClose,
  onSuccess,
}: InstallUrlDialogProps) {
  const [url, setUrl] = useState("");
  const [inspecting, setInspecting] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [inspection, setInspection] = useState<RemoteSourceInspection | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const targetAgents = agents.filter((agent) => agent.type !== "universal");
  const [selectedAgentIds, setSelectedAgentIds] = useState<string[]>([]);
  const [selectedSkillPaths, setSelectedSkillPaths] = useState<string[]>([]);
  const [selectedMcpNames, setSelectedMcpNames] = useState<string[]>([]);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");

  async function handleInspect() {
    if (!url.trim()) {
      setError("请输入 GitHub 仓库地址或本地路径");
      return;
    }
    setError(null);
    setSuccessMessage(null);
    setInspecting(true);
    try {
      const result = await api.inspectRemoteSource(url.trim());
      setInspection(result);
      // 默认全选检测到的所有技能
      setSelectedSkillPaths(result.skills.map((s) => s.relativePath || s.name));
      setSelectedMcpNames(result.mcpServers.map((mcp) => mcp.name));
      const recommended: string[] = [];
      if (result.mcpServers.length > 0) {
        const mcpAgent = result.availableAgents.find((agent) => agent.supportsMcp);
        if (mcpAgent && !recommended.includes(mcpAgent.id)) recommended.push(mcpAgent.id);
      }
      setSelectedAgentIds(recommended);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setInspecting(false);
    }
  }

  function toggleAgent(agentId: string) {
    setSelectedAgentIds((prev) =>
      prev.includes(agentId) ? prev.filter((id) => id !== agentId) : [...prev, agentId],
    );
  }

  function toggleSkill(relPath: string) {
    setSelectedSkillPaths((prev) =>
      prev.includes(relPath) ? prev.filter((p) => p !== relPath) : [...prev, relPath],
    );
  }

  function toggleMcp(name: string) {
    setSelectedMcpNames((prev) =>
      prev.includes(name) ? prev.filter((item) => item !== name) : [...prev, name],
    );
  }

  async function handleInstall() {
    if (!inspection) return;
    if (selectedSkillPaths.length === 0 && selectedMcpNames.length === 0) {
      setError("请至少选择一个 Skill 或 MCP 服务。");
      return;
    }
    const mcpAgentIds = inspection.availableAgents
      .filter((agent) => agent.supportsMcp && selectedAgentIds.includes(agent.id))
      .map((agent) => agent.id);
    if (selectedMcpNames.length > 0 && mcpAgentIds.length === 0) {
      setError("安装 MCP 服务需要选择至少一个支持 MCP 的 Agent，或取消勾选 MCP 服务。");
      return;
    }

    setError(null);
    setInstalling(true);
    const messages: string[] = [];
    try {
      if (selectedSkillPaths.length > 0) {
        const res = await api.installRemoteSource({
          url: inspection.url,
          targetAgentIds: selectedAgentIds,
          conflictPolicy,
          toHub: true,
          selectedSkills: selectedSkillPaths,
        });
        messages.push(res.message);
      }

      for (const mcp of inspection.mcpServers.filter((item) => selectedMcpNames.includes(item.name))) {
        await api.installRemoteMcp({
          config: {
            name: mcp.name,
            transport: mcp.transport,
            command: mcp.command,
            args: mcp.args,
            env: mcp.env,
            disabled: false,
          },
          targetAgentIds: mcpAgentIds,
          conflictPolicy,
        });
        messages.push(`已配置 MCP 服务: ${mcp.name}`);
      }
      setSuccessMessage(messages.join("；"));

      setTimeout(() => {
        onSuccess();
        onClose();
      }, 1200);
    } catch (e: unknown) {
      setError(messages.length > 0 ? `${messages.join("；")}；后续安装失败：${String(e)}` : String(e));
    } finally {
      setInstalling(false);
    }
  }

  return (
    <Dialog maxWidth={640} large onClose={onClose}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
        <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
          <svg className="icon" viewBox="0 0 24 24">
            <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
          </svg>
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>从 Git / GitHub URL 安装</h2>
          <p style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>
            支持 GitHub 仓库地址、子目录链接（/tree/main/...）或简写（owner/repo）
          </p>
        </div>
        <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
          <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
        </button>
      </div>

      <div style={{ flex: 1, overflow: "auto", padding: "20px 24px", display: "flex", flexDirection: "column", gap: 16 }}>
        {/* URL Input */}
        <div>
          <label style={{ display: "block", fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
            仓库地址或本地路径
          </label>
          <div style={{ display: "flex", gap: 8 }}>
            <input
              className="input"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="例如: https://github.com/anthropics/anthropic-quickstarts 或 owner/repo"
              style={{ flex: 1 }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  handleInspect();
                }
              }}
            />
            <button
              className="btn btn-primary btn-sm"
              onClick={handleInspect}
              disabled={inspecting || busy || !url.trim()}
              type="button"
              style={{ minWidth: 72 }}
            >
              {inspecting ? "识别中..." : "识别"}
            </button>
          </div>
        </div>

        {error && (
          <div style={{ padding: "10px 14px", background: "var(--error-light)", color: "var(--error)", borderRadius: "var(--radius-sm)", fontSize: 12 }}>
            {error}
          </div>
        )}

        {successMessage && (
          <div style={{ padding: "10px 14px", background: "var(--success-light)", color: "var(--success)", borderRadius: "var(--radius-sm)", fontSize: 12 }}>
            {successMessage}
          </div>
        )}

        {/* Inspection Result Preview */}
        {inspection && (
          <div style={{ background: "var(--surface-raised)", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", padding: 16 }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
              <div>
                <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text)" }}>{inspection.repoName}</span>
                {inspection.subpath && (
                  <span style={{ fontSize: 12, color: "var(--text-tertiary)", marginLeft: 6 }}>
                    ({inspection.subpath})
                  </span>
                )}
              </div>
              <span className="badge badge-accent">
                {inspection.detectedType === "singleSkill" && "单个 Skill"}
                {inspection.detectedType === "multiSkill" && `多技能集合 (${inspection.skills.length})`}
                {inspection.detectedType === "mcpServer" && "MCP 服务"}
                {inspection.detectedType === "both" && "Skill & MCP"}
                {inspection.detectedType === "unknown" && "未知内容"}
              </span>
            </div>

            {/* Skills List */}
            {inspection.skills.length > 0 && (
              <div style={{ marginTop: 8 }}>
                <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
                  包含技能 ({inspection.skills.length} 个):
                </p>
                <div style={{ display: "flex", flexDirection: "column", gap: 6, maxHeight: 180, overflowY: "auto" }}>
                  {inspection.skills.map((skill) => {
                    const key = skill.relativePath || skill.name;
                    const checked = selectedSkillPaths.includes(key);
                    return (
                      <div
                        key={key}
                        onClick={() => toggleSkill(key)}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 10,
                          padding: "6px 10px",
                          borderRadius: "var(--radius-sm)",
                          background: checked ? "var(--accent-soft)" : "var(--surface)",
                          border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                          cursor: "pointer",
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() => {}}
                          style={{ cursor: "pointer" }}
                        />
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ fontSize: 13, fontWeight: 500, color: "var(--text)" }}>
                            {skill.title}
                            {skill.version && (
                              <span style={{ fontSize: 11, color: "var(--text-tertiary)", marginLeft: 6 }}>
                                v{skill.version}
                              </span>
                            )}
                          </div>
                          {skill.description && (
                            <div style={{ fontSize: 11, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                              {skill.description}
                            </div>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            {/* MCP Servers List */}
            {inspection.mcpServers.length > 0 && (
              <div style={{ marginTop: 12 }}>
                <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 6 }}>
                  包含 MCP 服务 ({inspection.mcpServers.length} 个):
                </p>
                {inspection.mcpServers.map((mcp) => (
                  <div
                    key={mcp.name}
                    onClick={() => toggleMcp(mcp.name)}
                    style={{
                      cursor: "pointer",
                      padding: "8px 12px",
                      borderRadius: "var(--radius-sm)",
                      background: "var(--surface)",
                      border: "1px solid var(--border)",
                      fontSize: 12,
                    }}
                  >
                    <input type="checkbox" checked={selectedMcpNames.includes(mcp.name)} onChange={() => {}} style={{ marginRight: 8 }} />
                    <div style={{ fontWeight: 600, color: "var(--text)" }}>{mcp.name}</div>
                    <div style={{ color: "var(--text-secondary)", marginTop: 2 }}>{mcp.description}</div>
                    {mcp.command && (
                      <code style={{ fontSize: 11, display: "block", marginTop: 4, background: "var(--surface-raised)", padding: "2px 6px", borderRadius: 4 }}>
                        {mcp.command} {mcp.args.join(" ")}
                      </code>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Target Agent Selection */}
        {inspection && (
          <div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
              <label style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>
                分发到以下 Agent（可不选，仅保存到中枢）:
              </label>
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => {
                  if (selectedAgentIds.length === targetAgents.length) {
                    setSelectedAgentIds([]);
                  } else {
                    setSelectedAgentIds(targetAgents.map((a) => a.id));
                  }
                }}
                type="button"
                style={{ fontSize: 11, padding: "2px 8px" }}
              >
                {selectedAgentIds.length === targetAgents.length ? "取消全选" : "全选"}
              </button>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6, maxHeight: 150, overflowY: "auto" }}>
              {targetAgents.map((agent) => {
                const checked = selectedAgentIds.includes(agent.id);
                return (
                  <button
                    key={agent.id}
                    className={`agent-item${checked ? " selected" : ""}`}
                    onClick={() => toggleAgent(agent.id)}
                    type="button"
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 10,
                      padding: "8px 12px",
                      borderRadius: "var(--radius-sm)",
                      border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                      background: checked ? "var(--accent-soft)" : "var(--surface)",
                      textAlign: "left",
                      cursor: "pointer",
                      width: "100%",
                    }}
                  >
                    <input type="checkbox" checked={checked} onChange={() => {}} style={{ cursor: "pointer" }} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 13, fontWeight: 500, color: "var(--text)", display: "flex", alignItems: "center", gap: 6 }}>
                        {agent.name}
                      </div>
                      <div style={{ fontSize: 11, color: "var(--text-tertiary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {agent.skillsPath}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>

            {/* Options */}
            <div style={{ marginTop: 12, display: "flex", gap: 16, alignItems: "center" }}>
              <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>Skill 始终保存在程序中枢。</span>

              <div style={{ display: "flex", alignItems: "center", gap: 6, marginLeft: "auto", fontSize: 12 }}>
                <span style={{ color: "var(--text-secondary)" }}>冲突策略:</span>
                <select
                  className="input"
                  value={conflictPolicy}
                  onChange={(e) => setConflictPolicy(e.target.value as ConflictPolicy)}
                  style={{ fontSize: 12, padding: "3px 8px", width: "auto" }}
                >
                  {policyOptions.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          </div>
        )}
      </div>

      <div style={{ display: "flex", alignItems: "center", justifyContent: "flex-end", gap: 10, borderTop: "1px solid var(--border)", padding: "16px 24px" }}>
        <button className="btn btn-secondary" onClick={onClose} disabled={installing} type="button">
          取消
        </button>
        <button
          className="btn btn-primary"
          onClick={handleInstall}
          disabled={!inspection || installing || (selectedSkillPaths.length === 0 && selectedMcpNames.length === 0)}
          type="button"
        >
          {installing ? "正在安装..." : "确认安装"}
        </button>
      </div>
    </Dialog>
  );
}
