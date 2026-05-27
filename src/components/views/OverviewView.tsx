import { FolderArchive, FolderPlus } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "../ui/card";
import type { GroupedSkill, AgentProfile } from "../../types";

interface OverviewViewProps {
  skills: GroupedSkill[];
  agents: AgentProfile[];
  onNavigate: (view: "skills") => void;
  onFolder: () => void;
  onArchive: () => void;
}

export function OverviewView({ skills, agents, onNavigate, onFolder, onArchive }: OverviewViewProps) {
  const missing = skills.reduce((t, s) => t + s.missingAgentIds.length, 0);

  return (
    <div className="flex flex-col gap-5">
      <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Skills</CardDescription>
            <CardTitle className="text-2xl">{skills.length}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-xs text-[var(--color-text-secondary)]">按标题去重后的项目</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>Agents</CardDescription>
            <CardTitle className="text-2xl">{agents.length}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-xs text-[var(--color-text-secondary)]">已识别的本地配置</p>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardDescription>缺失副本</CardDescription>
            <CardTitle className="text-2xl">{missing}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-xs text-[var(--color-text-secondary)]">Agent 缺失的 skill 副本</p>
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>快速操作</CardTitle>
          <CardDescription>导入或查看本地 skills</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <button
              className="flex min-h-28 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] p-5 text-[var(--color-text-secondary)] transition-[background,border-color,color] hover:border-[var(--color-accent)] hover:bg-[var(--color-accent-soft)] hover:text-[var(--color-accent)]"
              onClick={onFolder}
              type="button"
            >
              <FolderPlus size={20} />
              <span className="text-xs font-medium">导入文件夹</span>
            </button>
            <button
              className="flex min-h-28 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] p-5 text-[var(--color-text-secondary)] transition-[background,border-color,color] hover:border-[var(--color-accent)] hover:bg-[var(--color-accent-soft)] hover:text-[var(--color-accent)]"
              onClick={onArchive}
              type="button"
            >
              <FolderArchive size={20} />
              <span className="text-xs font-medium">导入 zip</span>
            </button>
            <button
              className="flex min-h-28 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] p-5 text-[var(--color-text-secondary)] transition-[background,border-color,color] hover:border-[var(--color-accent)] hover:bg-[var(--color-accent-soft)] hover:text-[var(--color-accent)]"
              onClick={() => onNavigate("skills")}
              type="button"
            >
              <FolderPlus size={20} />
              <span className="text-xs font-medium">查看 Skills</span>
            </button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
