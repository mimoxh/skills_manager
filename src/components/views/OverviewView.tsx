import type { GroupedSkill, AgentProfile } from "../../types";

interface OverviewViewProps {
  skills: GroupedSkill[];
  agents: AgentProfile[];
  noFullCoverageTitles: Set<string>;
  onNavigate: (view: "skills" | "agents", filter?: "all" | "covered" | "partial" | "needed") => void;
  onFolder: () => void;
  onArchive: () => void;
}

export function OverviewView({ skills, agents, noFullCoverageTitles, onNavigate, onFolder, onArchive }: OverviewViewProps) {
  const missing = skills.filter((s) => s.missingAgentIds.length > 0 && !noFullCoverageTitles.has(s.title)).length;

  return (
    <>
      {/* Stats */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14, marginBottom: 24 }}>
        <div className="card" onClick={() => onNavigate("skills")} style={{ cursor: "pointer" }}>
          <div className="card-header">
            <div className="card-desc">Skills</div>
            <div className="card-title" style={{ fontSize: 28, marginTop: 4 }}>{skills.length}</div>
          </div>
        </div>
        <div className="card" onClick={() => onNavigate("agents")} style={{ cursor: "pointer" }}>
          <div className="card-header">
            <div className="card-desc">Agents</div>
            <div className="card-title" style={{ fontSize: 28, marginTop: 4 }}>{agents.length}</div>
          </div>
        </div>
        <div className="card" onClick={() => onNavigate("skills", "needed")} style={{ cursor: "pointer" }}>
          <div className="card-header">
            <div className="card-desc">需同步</div>
            <div className="card-title" style={{ fontSize: 28, marginTop: 4, color: "var(--warning)" }}>{missing}</div>
          </div>
        </div>
      </div>

      {/* Quick Actions */}
      <div className="card">
        <div className="card-header">
          <div className="card-title">快速操作</div>
          <div className="card-desc">导入或查看本地 skills</div>
        </div>
        <div className="card-body">
          <div className="quick-actions">
            <button className="quick-action" onClick={onFolder} type="button">
              <div className="quick-action-icon">
                <svg className="icon icon-lg" viewBox="0 0 24 24"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
              </div>
              <span className="quick-action-label">导入文件夹</span>
            </button>
            <button className="quick-action" onClick={onArchive} type="button">
              <div className="quick-action-icon">
                <svg className="icon icon-lg" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" /><polyline points="7 10 12 15 17 10" /><line x1="12" y1="15" x2="12" y2="3" /></svg>
              </div>
              <span className="quick-action-label">导入 zip</span>
            </button>
            <button className="quick-action" onClick={() => onNavigate("skills")} type="button">
              <div className="quick-action-icon">
                <svg className="icon icon-lg" viewBox="0 0 24 24"><polygon points="12 2 2 7 12 12 22 7 12 2" /><polyline points="2 17 12 22 22 17" /><polyline points="2 12 12 17 22 12" /></svg>
              </div>
              <span className="quick-action-label">查看 Skills</span>
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
