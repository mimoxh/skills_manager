import { useState } from "react";
import type { AgentProfile, ConflictPolicy } from "../../types";

const policyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新目标目录" },
  { value: "skip", label: "跳过冲突", helper: "目标已存在时不做修改" },
  { value: "rename", label: "另存副本", helper: "生成带时间戳的新副本" },
];

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
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ maxHeight: "88vh", width: "100%", maxWidth: 560, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
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

        <div style={{ flex: 1, overflow: "auto", padding: "20px 24px", display: "flex", flexDirection: "column", gap: 20 }}>
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

        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, borderTop: "1px solid var(--border)", background: "var(--surface-raised)", padding: "16px 24px" }}>
          <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>已选择 {selectedAgents.length} 个 Agent</p>
          <div style={{ display: "flex", gap: 8 }}>
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
