import { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import type { AgentProfile, AgentSkillCopy, ConflictPolicy } from "../../types";
import { UserTagEditor } from "./UserTagEditor";

const policyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新目标目录" },
  { value: "skip", label: "跳过冲突", helper: "目标已存在时不做修改" },
  { value: "rename", label: "另存副本", helper: "生成带时间戳的新副本" },
];

interface SkillInstallDialogProps {
  agents: AgentProfile[];
  allowNoTargets?: boolean;
  availableUserTags?: string[];
  busy: boolean;
  conflictPolicy: ConflictPolicy;
  description?: string | null;
  enableAgentTagFilter?: boolean;
  installedAgentIds?: string[];
  isNoFullCoverage?: boolean;
  metadata?: Array<{ label: string; value?: string | number | null }>;
  primaryLabel: string;
  readme?: string | null;
  selectedAgentIds: string[];
  selectedSourceAgentId?: string | null;
  sourceCopies?: AgentSkillCopy[];
  sourceLabel?: string | null;
  tags?: string[];
  title: string;
  version?: string | null;
  onClose: () => void;
  onConfirm: () => void;
  onPolicy: (policy: ConflictPolicy) => void;
  onSourceAgent?: (agentId: string) => void;
  onToggleAgent: (agentId: string) => void;
  onToggleNoFullCoverage?: () => void;
  onUserTagsChange?: (title: string, tags: string[]) => Promise<string[]>;
}

export function SkillInstallDialog({
  agents,
  allowNoTargets = false,
  availableUserTags = [],
  busy,
  conflictPolicy,
  description,
  enableAgentTagFilter = false,
  installedAgentIds = [],
  isNoFullCoverage = false,
  metadata = [],
  primaryLabel,
  readme,
  selectedAgentIds,
  selectedSourceAgentId,
  sourceCopies = [],
  sourceLabel,
  tags = [],
  title,
  version,
  onClose,
  onConfirm,
  onPolicy,
  onSourceAgent,
  onToggleAgent,
  onToggleNoFullCoverage,
  onUserTagsChange,
}: SkillInstallDialogProps) {
  const trimmedDescription = description?.trim();
  const trimmedReadme = readme?.trim();
  const hasReadableContent = Boolean(trimmedDescription || trimmedReadme);
  const [selectedAgentTagFilters, setSelectedAgentTagFilters] = useState<string[]>([]);

  const agentTagOptions = useMemo(() => {
    if (!enableAgentTagFilter) return [];
    const tagMap = new Map<string, string>();
    for (const agent of agents) {
      for (const tag of agent.userTags ?? []) {
        const key = tag.toLowerCase();
        if (!tagMap.has(key)) tagMap.set(key, tag);
      }
    }
    return [...tagMap.values()].sort((a, b) => a.localeCompare(b));
  }, [agents, enableAgentTagFilter]);

  const displayedAgents = useMemo(() => {
    if (!enableAgentTagFilter || selectedAgentTagFilters.length === 0) return agents;
    return agents.filter((agent) => matchesAgentTagFilters(agent, selectedAgentTagFilters));
  }, [agents, enableAgentTagFilter, selectedAgentTagFilters]);

  async function saveTags(nextTags: string[]) {
    if (!onUserTagsChange) return;
    await onUserTagsChange(title, nextTags);
  }

  function toggleAgentTagFilter(tag: string) {
    setSelectedAgentTagFilters((current) =>
      current.some((value) => value.toLowerCase() === tag.toLowerCase())
        ? current.filter((value) => value.toLowerCase() !== tag.toLowerCase())
        : [...current, tag],
    );
  }

  return (
    <div onClick={onClose} style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(47, 48, 44, 0.28)", padding: 20 }}>
      <div onClick={(event) => event.stopPropagation()} style={{ maxHeight: "88vh", width: "100%", maxWidth: 980, display: "flex", flexDirection: "column", overflow: "hidden", borderRadius: "var(--radius-lg)", border: "1px solid var(--border)", background: "var(--surface-raised)", boxShadow: "0 18px 55px rgba(80,60,30,0.14), 0 2px 8px rgba(80,60,30,0.06)" }}>
        <div style={{ display: "flex", alignItems: "flex-start", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
          <div style={{ width: 40, height: 40, background: "var(--accent-light)", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--accent)", flexShrink: 0 }}>
            <svg className="icon" viewBox="0 0 24 24"><polygon points="12 2 2 7 12 12 22 7 12 2" /><polyline points="2 17 12 22 22 17" /><polyline points="2 12 12 17 22 12" /></svg>
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)", overflowWrap: "anywhere" }}>{title}</h2>
            <p style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>
              {[sourceLabel, version ? `v${version}` : null].filter(Boolean).join(" · ") || "Skill 信息"}
            </p>
          </div>
          <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
            <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
          </button>
        </div>

        <div style={{ flex: 1, overflow: "hidden", display: "grid", gridTemplateColumns: "1.2fr 1fr" }}>
          <div style={{ overflow: "auto", padding: "20px 24px", borderRight: "1px solid var(--border)" }}>
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 14 }}>
              {version && <span className="badge badge-version">v{version}</span>}
              {tags.map((tag) => <span className="badge badge-user-tag" key={tag}>{tag}</span>)}
            </div>

            {onUserTagsChange && (
              <div className="detail-section">
                <p className="detail-section-title">自定义标签</p>
                <UserTagEditor availableTags={availableUserTags} busy={busy} onChange={saveTags} tags={tags} />
              </div>
            )}

            {metadata.filter((item) => item.value !== null && item.value !== undefined && item.value !== "").length > 0 && (
              <div className="detail-section">
                <p className="detail-section-title">Skill 信息</p>
                {metadata.map((item) => (
                  item.value !== null && item.value !== undefined && item.value !== "" ? (
                    <div className="detail-item" key={item.label}>
                      <span style={{ fontSize: 12, color: "var(--text-secondary)", minWidth: 76 }}>{item.label}</span>
                      <code style={{ fontFamily: "var(--font-mono)", fontSize: 12, wordBreak: "break-all" }}>{item.value}</code>
                    </div>
                  ) : null
                ))}
              </div>
            )}

            <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 12 }}>说明</p>
            {hasReadableContent ? (
              <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
                {trimmedDescription && (
                  <p style={{ fontSize: 13, lineHeight: 1.7, color: "var(--text)", margin: 0, whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
                    {trimmedDescription}
                  </p>
                )}
                {trimmedReadme && (
                  <div className="markdown-body">
                    <ReactMarkdown>{trimmedReadme}</ReactMarkdown>
                  </div>
                )}
              </div>
            ) : (
              <p style={{ fontSize: 13, color: "var(--text-tertiary)", fontStyle: "italic" }}>暂无说明</p>
            )}
          </div>

          <div style={{ overflow: "auto", padding: "20px 24px", display: "flex", flexDirection: "column", gap: 20 }}>
            <div>
              <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>目标 Agent</p>
              {enableAgentTagFilter && agentTagOptions.length > 0 && (
                <div className="skills-tag-filter-row" style={{ padding: "0 0 10px" }}>
                  <span className="skills-tag-filter-label">标签</span>
                  {agentTagOptions.map((tag) => {
                    const selected = selectedAgentTagFilters.some((value) => value.toLowerCase() === tag.toLowerCase());
                    return (
                      <button
                        className={`badge badge-user-tag skill-tag-filter${selected ? " selected" : ""}`}
                        key={tag}
                        onClick={() => toggleAgentTagFilter(tag)}
                        type="button"
                      >
                        {tag}
                      </button>
                    );
                  })}
                  {selectedAgentTagFilters.length > 0 && (
                    <button className="skills-tag-filter-clear" onClick={() => setSelectedAgentTagFilters([])} type="button">
                      清空标签筛选
                    </button>
                  )}
                </div>
              )}
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {displayedAgents.map((agent) => {
                  const checked = selectedAgentIds.includes(agent.id);
                  const installed = installedAgentIds.includes(agent.id);
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
                      <span style={{ flex: 1, minWidth: 0, textAlign: "left" }}>
                        <span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{agent.name}</span>
                        <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{agent.skillsPath}</span>
                        {(agent.userTags ?? []).length > 0 && (
                          <span className="agent-tags">
                            {(agent.userTags ?? []).map((tag) => (
                              <span className="badge badge-user-tag" key={tag}>{tag}</span>
                            ))}
                          </span>
                        )}
                      </span>
                      {installed && <span className="badge badge-success">已安装</span>}
                    </button>
                  );
                })}
                {!agents.length && (
                  <p style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "12px 0" }}>暂无 Agent，请先添加 Agent。</p>
                )}
                {agents.length > 0 && !displayedAgents.length && (
                  <p style={{ fontSize: 12, color: "var(--text-tertiary)", padding: "12px 0" }}>当前标签筛选条件下没有匹配的 Agent。</p>
                )}
              </div>
            </div>

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

            {sourceCopies.length > 1 && onSourceAgent && (
              <div>
                <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)", marginBottom: 8 }}>来源副本</p>
                <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                  {sourceCopies.map((copy) => {
                    const checked = selectedSourceAgentId === copy.agentId;
                    return (
                      <button
                        key={`${copy.agentId}:${copy.skillPath}`}
                        className={`agent-item${checked ? " selected" : ""}`}
                        onClick={() => onSourceAgent(copy.agentId)}
                        type="button"
                        style={checked ? { borderColor: "var(--accent)", background: "var(--accent-soft)" } : undefined}
                      >
                        <span style={{
                          width: 20, height: 20, flexShrink: 0, borderRadius: 10, border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`,
                          background: checked ? "var(--accent)" : "transparent", display: "flex", alignItems: "center", justifyContent: "center",
                        }}>
                          {checked && <svg width="10" height="10" viewBox="0 0 24 24" fill="white"><circle cx="12" cy="12" r="7" /></svg>}
                        </span>
                        <span style={{ flex: 1, minWidth: 0, textAlign: "left" }}>
                          <span style={{ display: "block", fontSize: 14, fontWeight: 500 }}>{copy.agentName}</span>
                          <span style={{ display: "block", fontSize: 12, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{copy.skillPath}</span>
                        </span>
                        <span className="badge badge-version">{copy.version ? `v${copy.version}` : "无版本"}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}
          </div>
        </div>

        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, borderTop: "1px solid var(--border)", background: "var(--surface-raised)", padding: "16px 24px" }}>
          <p style={{ fontSize: 12, color: "var(--text-secondary)" }}>已选择 {selectedAgentIds.length} 个 Agent</p>
          <div style={{ display: "flex", gap: 8 }}>
            {onToggleNoFullCoverage && (
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
            )}
            <button className="btn btn-primary" onClick={onConfirm} disabled={busy || (!allowNoTargets && selectedAgentIds.length === 0)} type="button">
              <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="20 6 9 17 4 12" /></svg>
              {primaryLabel}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function matchesAgentTagFilters(agent: AgentProfile, selectedTags: string[]): boolean {
  if (!selectedTags.length) return true;
  const tags = new Set((agent.userTags ?? []).map((tag) => tag.toLowerCase()));
  return selectedTags.every((tag) => tags.has(tag.toLowerCase()));
}
