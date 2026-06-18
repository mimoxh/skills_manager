import { useMemo, useState } from "react";
import type { AgentProfile, AgentSkillCopy, ConflictPolicy, GroupedSkill, InstallResult } from "../../types";
import { SkillInstallDialog } from "./SkillInstallDialog";

interface SkillsViewProps {
  skills: GroupedSkill[];
  agents: AgentProfile[];
  busy: boolean;
  noFullCoverageTitles: Set<string>;
  initialFilter?: "all" | "covered" | "partial" | "needed";
  onDrop: (event: React.DragEvent<HTMLElement>) => void;
  onFolder: () => void;
  onArchive: () => void;
  onSync: (title: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy, sourceAgentId?: string | null) => Promise<InstallResult[]>;
  onUninstall: (skillId: string, agentIds: string[]) => Promise<void>;
  onLoadReadme: (skillPath: string) => Promise<string | null>;
  onRefresh: () => void;
  onToggleNoFullCoverage: (title: string) => Promise<void>;
}

export function SkillsView({ skills, agents, busy, noFullCoverageTitles, initialFilter = "all", onDrop, onFolder, onArchive, onSync, onUninstall, onLoadReadme, onRefresh, onToggleNoFullCoverage }: SkillsViewProps) {
  const [selectedSkill, setSelectedSkill] = useState<GroupedSkill | null>(null);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [selectedSourceAgentId, setSelectedSourceAgentId] = useState<string | null>(null);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");
  const [lastResults, setLastResults] = useState<InstallResult[]>([]);
  const [dragging, setDragging] = useState(false);
  const [filter, setFilter] = useState<"all" | "covered" | "partial" | "needed">(initialFilter);
  const [deleteTarget, setDeleteTarget] = useState<GroupedSkill | null>(null);
  const [discardConfirm, setDiscardConfirm] = useState(false);

  const displayedSkills = useMemo(() => {
    if (filter === "covered") return skills.filter((s) => s.missingAgentIds.length === 0);
    if (filter === "partial") return skills.filter((s) => s.missingAgentIds.length > 0 && noFullCoverageTitles.has(s.title));
    if (filter === "needed") return skills.filter((s) => s.missingAgentIds.length > 0 && !noFullCoverageTitles.has(s.title));
    return skills;
  }, [skills, filter, noFullCoverageTitles]);

  const coveredCount = useMemo(() => {
    return skills.filter((s) => s.missingAgentIds.length === 0).length;
  }, [skills]);

  const partialCount = useMemo(() => {
    return skills.filter((s) => s.missingAgentIds.length > 0 && noFullCoverageTitles.has(s.title)).length;
  }, [skills, noFullCoverageTitles]);

  const neededCount = useMemo(() => {
    return skills.filter((s) => s.missingAgentIds.length > 0 && !noFullCoverageTitles.has(s.title)).length;
  }, [skills, noFullCoverageTitles]);

  async function openSync(skill: GroupedSkill) {
    const sourceCopy = preferredSourceCopy(skill);
    setSelectedSkill(skill);
    setSelectedSourceAgentId(sourceCopy.agentId);
    setSelectedAgents(skill.installedAgentIds);
    setConflictPolicy("backupOverwrite");
    setLastResults([]);
    if (!sourceCopy.readme) {
      const readme = await onLoadReadme(sourceCopy.skillPath);
      if (readme) {
        setSelectedSkill((current) =>
          current?.title === skill.title
            ? {
                ...current,
                readme,
                bestCopy: current.bestCopy.agentId === sourceCopy.agentId ? { ...current.bestCopy, readme } : current.bestCopy,
                copies: current.copies.map((copy) => copy.agentId === sourceCopy.agentId ? { ...copy, readme } : copy),
              }
            : current,
        );
      }
    }
  }

  async function selectSourceAgent(agentId: string) {
    if (!selectedSkill) return;
    setSelectedSourceAgentId(agentId);
    const copy = selectedSkill.copies.find((candidate) => candidate.agentId === agentId);
    if (!copy || copy.readme) return;
    const readme = await onLoadReadme(copy.skillPath);
    if (readme) {
      setSelectedSkill((current) =>
        current?.title === selectedSkill.title
          ? {
              ...current,
              readme: current.bestCopy.agentId === agentId ? readme : current.readme,
              bestCopy: current.bestCopy.agentId === agentId ? { ...current.bestCopy, readme } : current.bestCopy,
              copies: current.copies.map((candidate) => candidate.agentId === agentId ? { ...candidate, readme } : candidate),
            }
          : current,
      );
    }
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
      const results = await onSync(selectedSkill.title, selectedAgents, conflictPolicy, selectedSourceAgentId);
      setLastResults(results);
    }
    setSelectedSkill(null);
    setSelectedSourceAgentId(null);
  }

  async function confirmDelete() {
    if (!deleteTarget) return;
    await onUninstall(deleteTarget.title, deleteTarget.installedAgentIds);
    setDeleteTarget(null);
  }

  const hasSyncChanges = useMemo(() => {
    if (!selectedSkill) return false;
    const initial = selectedSkill.installedAgentIds;
    if (selectedAgents.length !== initial.length || selectedAgents.some((id) => !initial.includes(id))) return true;
    if (selectedSourceAgentId && selectedSourceAgentId !== preferredSourceCopy(selectedSkill).agentId) return true;
    if (conflictPolicy !== "backupOverwrite") return true;
    return false;
  }, [selectedSkill, selectedAgents, selectedSourceAgentId, conflictPolicy]);

  function requestClose() {
    if (hasSyncChanges) {
      setDiscardConfirm(true);
    } else {
      setSelectedSkill(null);
      setSelectedSourceAgentId(null);
    }
  }

  return (
    <>
      {/* Metrics */}
      <div className="metrics">
        <div
          className="metric-card"
          onClick={() => setFilter("all")}
          style={{ cursor: filter !== "all" ? "pointer" : "default" }}
        >
          <div className="metric-value">{skills.length}</div>
          <div className="metric-label">Skills</div>
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
                  {skill.missingAgentIds.length > 0 && !noFullCoverageTitles.has(skill.title) && (
                    <span className="badge badge-warning">{skill.missingAgentIds.length} 缺失</span>
                  )}
                  {noFullCoverageTitles.has(skill.title) && (
                    <span className="badge badge-muted">无需全覆盖</span>
                  )}
                </div>
              </div>
              <span className={`badge ${skill.missingAgentIds.length > 0 && !noFullCoverageTitles.has(skill.title) ? "badge-syncable" : "badge-synced"}`}>
                {skill.missingAgentIds.length > 0 && !noFullCoverageTitles.has(skill.title) ? "需同步" : noFullCoverageTitles.has(skill.title) ? "已部分覆盖" : "已覆盖"}
              </span>
              <button
                className="btn-icon"
                title={noFullCoverageTitles.has(skill.title) ? "取消无需全覆盖" : "标记无需全覆盖"}
                style={{ width: 32, height: 32, flexShrink: 0, ...(noFullCoverageTitles.has(skill.title) ? { color: "var(--accent)" } : {}) }}
                onClick={(e) => { e.stopPropagation(); onToggleNoFullCoverage(skill.title); }}
                disabled={busy}
                type="button"
              >
                <svg className="icon icon-sm" viewBox="0 0 24 24"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" /></svg>
              </button>
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
        <SkillInstallDialog
          allowNoTargets
          agents={agents}
          busy={busy}
          conflictPolicy={conflictPolicy}
          description={selectedSourceCopy(selectedSkill, selectedSourceAgentId).description || selectedSkill.description}
          installedAgentIds={selectedSkill.installedAgentIds}
          selectedAgentIds={selectedAgents}
          selectedSourceAgentId={selectedSourceAgentId}
          sourceCopies={selectedSkill.copies}
          sourceLabel={`来源 ${selectedSourceCopy(selectedSkill, selectedSourceAgentId).agentName} · ${selectedSkill.copies.length} 个副本`}
          metadata={[
            { label: "来源路径", value: selectedSourceCopy(selectedSkill, selectedSourceAgentId).skillPath },
            { label: "来源 Agent", value: selectedSourceCopy(selectedSkill, selectedSourceAgentId).agentName },
            { label: "副本数量", value: selectedSkill.copies.length },
          ]}
          primaryLabel={selectedAgents.length === 0 ? "全部删除" : selectedAgents.length < selectedSkill.installedAgentIds.length ? "同步并清理" : "同步"}
          readme={selectedSourceCopy(selectedSkill, selectedSourceAgentId).readme || selectedSkill.readme || selectedSkill.bestCopy.readme}
          title={selectedSkill.title}
          version={selectedSourceCopy(selectedSkill, selectedSourceAgentId).version}
          isNoFullCoverage={noFullCoverageTitles.has(selectedSkill.title)}
          onClose={requestClose}
          onPolicy={setConflictPolicy}
          onSourceAgent={selectSourceAgent}
          onToggleAgent={toggleAgent}
          onConfirm={executeSync}
          onToggleNoFullCoverage={async () => { await onToggleNoFullCoverage(selectedSkill.title); setSelectedSkill(null); }}
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

      {discardConfirm && (
        <ConfirmDialog
          title="放弃更改"
          message="当前有未保存的更改，确定要放弃吗？"
          confirmLabel="放弃"
          busy={busy}
          onClose={() => setDiscardConfirm(false)}
          onConfirm={() => { setDiscardConfirm(false); setSelectedSkill(null); setSelectedSourceAgentId(null); }}
        />
      )}
    </>
  );
}

function preferredSourceCopy(skill: GroupedSkill): AgentSkillCopy {
  return skill.copies.find((copy) => copy.agentId.startsWith("codex:")) || skill.bestCopy;
}

function selectedSourceCopy(skill: GroupedSkill, agentId: string | null): AgentSkillCopy {
  return skill.copies.find((copy) => copy.agentId === agentId) || preferredSourceCopy(skill);
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
