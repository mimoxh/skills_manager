import { useMemo, useState } from "react";
import {
  Archive,
  Check,
  FolderPlus,
  Layers3,
  RefreshCw,
  UploadCloud,
  X,
} from "lucide-react";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "../ui/card";
import { cn } from "../../lib/utils";
import type {
  AgentProfile,
  ConflictPolicy,
  GroupedSkill,
  InstallResult,
} from "../../types";

const policyOptions: Array<{ value: ConflictPolicy; label: string; helper: string }> = [
  { value: "backupOverwrite", label: "备份覆盖", helper: "保留备份后更新目标目录" },
  { value: "skip", label: "跳过冲突", helper: "目标已存在时不做修改" },
  { value: "rename", label: "另存副本", helper: "生成带时间戳的新副本" },
];

interface SkillsViewProps {
  skills: GroupedSkill[];
  agents: AgentProfile[];
  busy: boolean;
  onDrop: (event: React.DragEvent<HTMLElement>) => void;
  onDrag: (dragging: boolean) => void;
  dragging: boolean;
  onFolder: () => void;
  onArchive: () => void;
  onSync: (
    title: string,
    targetAgentIds: string[],
    conflictPolicy: ConflictPolicy,
  ) => Promise<InstallResult[]>;
}

export function SkillsView({
  skills,
  agents,
  busy,
  onDrop,
  onDrag,
  dragging,
  onFolder,
  onArchive,
  onSync,
}: SkillsViewProps) {
  const [selectedSkill, setSelectedSkill] = useState<GroupedSkill | null>(null);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");
  const [lastResults, setLastResults] = useState<InstallResult[]>([]);

  const totals = useMemo(() => {
    return skills.reduce(
      (acc, skill) => {
        acc.installed += skill.installedAgentIds.length;
        acc.missing += skill.missingAgentIds.length;
        acc.copies += skill.copies.length;
        return acc;
      },
      { installed: 0, missing: 0, copies: 0 },
    );
  }, [skills]);

  function openSync(skill: GroupedSkill) {
    setSelectedSkill(skill);
    setSelectedAgents(skill.missingAgentIds);
    setConflictPolicy("backupOverwrite");
    setLastResults([]);
  }

  function toggleAgent(agentId: string) {
    setSelectedAgents((prev) =>
      prev.includes(agentId) ? prev.filter((id) => id !== agentId) : [...prev, agentId],
    );
  }

  async function executeSync() {
    if (!selectedSkill) return;
    const results = await onSync(selectedSkill.title, selectedAgents, conflictPolicy);
    setLastResults(results);
    setSelectedSkill(null);
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-5">
      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Metric label="Skills" value={skills.length} />
        <Metric label="副本" value={totals.copies} />
        <Metric label="已安装" value={totals.installed} tone="success" />
        <Metric label="缺失" value={totals.missing} tone="warning" />
      </div>

      <div
        className={cn(
          "flex min-h-[76px] items-center gap-4 rounded-lg border border-dashed px-5 py-4 transition-[background,border-color,box-shadow]",
          dragging
            ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] shadow-[var(--shadow-soft)]"
            : "border-[var(--color-border)] bg-[var(--color-surface-raised)]",
        )}
        onDragOver={(e) => {
          e.preventDefault();
          onDrag(true);
        }}
        onDragLeave={() => onDrag(false)}
        onDrop={onDrop}
      >
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-[var(--color-surface)] text-[var(--color-text-secondary)]">
          <UploadCloud size={18} />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold text-[var(--color-text)]">导入 Skill</p>
          <p className="mt-1 text-xs leading-relaxed text-[var(--color-text-secondary)]">
            拖拽文件夹或 zip 到这里，或选择本地文件
          </p>
        </div>
        <Button variant="secondary" size="sm" onClick={onFolder} disabled={busy}>
          <FolderPlus size={14} />
          文件夹
        </Button>
        <Button variant="secondary" size="sm" onClick={onArchive} disabled={busy}>
          <Archive size={14} />
          zip
        </Button>
      </div>

      <Card className="flex min-h-0 flex-1 flex-col">
        <CardHeader className="shrink-0">
          <div className="flex items-center justify-between gap-3">
            <div>
              <CardTitle>Skills 控制台</CardTitle>
              <CardDescription>点击任意 skill 选择目标 Agent 同步</CardDescription>
            </div>
            <Badge variant="secondary">{skills.length} 个可见项目</Badge>
          </div>
        </CardHeader>
        <CardContent className="min-h-0 flex-1 overflow-auto px-4 pb-4">
          <div className="flex flex-col gap-2.5">
            {skills.map((skill) => (
              <button
                key={skill.title}
                className="group flex w-full items-center gap-4 rounded-md border border-transparent bg-[var(--color-surface-raised)] px-3 py-3 text-left transition-[background,border-color,box-shadow] hover:border-[var(--color-border-hover)] hover:bg-[var(--color-surface)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[color-mix(in_srgb,var(--color-accent)_24%,transparent)]"
                onClick={() => openSync(skill)}
                type="button"
              >
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-[var(--color-accent-light)] text-[var(--color-accent)]">
                  <Layers3 size={16} />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-semibold text-[var(--color-text)]">{skill.title}</p>
                  <p className="mt-1 truncate text-xs text-[var(--color-text-secondary)]">
                    来源 {skill.bestCopy.agentName} · {skill.copies.length} 个副本
                  </p>
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    <Badge variant="secondary">
                      {skill.bestCopy.version ? `v${skill.bestCopy.version}` : "未声明版本"}
                    </Badge>
                    <Badge variant="success">{skill.installedAgentIds.length} 已有</Badge>
                    {skill.missingAgentIds.length > 0 && (
                      <Badge variant="warning">{skill.missingAgentIds.length} 缺失</Badge>
                    )}
                  </div>
                </div>
                <Badge
                  className="shrink-0"
                  variant={skill.missingAgentIds.length > 0 ? "warning" : "success"}
                >
                  {skill.missingAgentIds.length > 0 ? "可同步" : "已覆盖"}
                </Badge>
              </button>
            ))}
            {!skills.length && (
              <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] py-12 text-center">
                <p className="text-sm font-medium text-[var(--color-text-secondary)]">
                  没有找到 skills
                </p>
                <p className="text-xs text-[var(--color-text-tertiary)]">
                  设置主仓库或导入包含 manifest 的文件夹后会显示在这里
                </p>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {lastResults.length > 0 && (
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-raised)] px-4 py-3 text-xs text-[var(--color-text-secondary)] shadow-sm">
          最近同步完成 {lastResults.length} 个任务
        </div>
      )}

      {selectedSkill && (
        <SyncSkillDialog
          agents={agents}
          busy={busy}
          conflictPolicy={conflictPolicy}
          selectedAgents={selectedAgents}
          skill={selectedSkill}
          onClose={() => setSelectedSkill(null)}
          onPolicy={setConflictPolicy}
          onSync={executeSync}
          onToggleAgent={toggleAgent}
        />
      )}
    </div>
  );
}

function Metric({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: "success" | "warning";
}) {
  const color =
    tone === "success"
      ? "text-[var(--color-success)]"
      : tone === "warning"
        ? "text-[var(--color-warning)]"
        : "text-[var(--color-text)]";
  return (
    <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-raised)] px-4 py-3 shadow-sm">
      <p className={cn("text-2xl font-semibold leading-none", color)}>{value}</p>
      <p className="mt-2 text-xs text-[var(--color-text-secondary)]">{label}</p>
    </div>
  );
}

function SyncSkillDialog({
  agents,
  busy,
  conflictPolicy,
  selectedAgents,
  skill,
  onClose,
  onPolicy,
  onSync,
  onToggleAgent,
}: {
  agents: AgentProfile[];
  busy: boolean;
  conflictPolicy: ConflictPolicy;
  selectedAgents: string[];
  skill: GroupedSkill;
  onClose: () => void;
  onPolicy: (policy: ConflictPolicy) => void;
  onSync: () => void;
  onToggleAgent: (agentId: string) => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[rgb(47_48_44_/_0.28)] px-5">
      <div className="flex max-h-[88vh] w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-[var(--color-border)] bg-[var(--color-surface-raised)] shadow-[var(--shadow-float)]">
        <div className="flex items-start gap-3 border-b border-[var(--color-border)] px-6 py-5">
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-[var(--color-accent-light)] text-[var(--color-accent)]">
            <RefreshCw size={16} />
          </div>
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-base font-semibold text-[var(--color-text)]">{skill.title}</h2>
            <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
              来源 {skill.bestCopy.agentName} · {skill.copies.length} 个副本
            </p>
          </div>
          <button
            className="flex h-9 w-9 items-center justify-center rounded-md text-[var(--color-text-secondary)] transition-colors hover:bg-[var(--color-surface)] hover:text-[var(--color-text)]"
            onClick={onClose}
            type="button"
            title="关闭"
          >
            <X size={15} />
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-auto px-6 py-5">
          <div className="mb-5">
            <p className="mb-2 text-xs font-semibold text-[var(--color-text-secondary)]">
              目标 Agent
            </p>
            <div className="flex flex-col gap-2.5">
              {agents.map((agent) => {
                const checked = selectedAgents.includes(agent.id);
                const installed = skill.installedAgentIds.includes(agent.id);
                return (
                  <button
                    key={agent.id}
                    className={cn(
                      "flex min-h-[68px] items-center gap-3 rounded-md border px-3 py-3 text-left transition-[background,border-color,box-shadow]",
                      checked
                        ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] shadow-sm"
                        : "border-[var(--color-border)] bg-[var(--color-surface-raised)] hover:bg-[var(--color-surface)]",
                    )}
                    onClick={() => onToggleAgent(agent.id)}
                    type="button"
                  >
                    <span
                      className={cn(
                        "flex h-5 w-5 shrink-0 items-center justify-center rounded border",
                        checked
                          ? "border-[var(--color-accent)] bg-[var(--color-accent)] text-white"
                          : "border-[var(--color-border)]",
                      )}
                    >
                      {checked && <Check size={13} />}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium">{agent.name}</span>
                      <span className="block truncate text-xs text-[var(--color-text-secondary)]">
                        {agent.skillsPath}
                      </span>
                    </span>
                    <Badge variant={installed ? "success" : "warning"}>
                      {installed ? "已安装" : "未安装"}
                    </Badge>
                  </button>
                );
              })}
              {!agents.length && (
                <p className="rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] py-8 text-center text-sm text-[var(--color-text-secondary)]">
                  没有可用 Agent
                </p>
              )}
            </div>
          </div>

          <div>
            <p className="mb-2 text-xs font-semibold text-[var(--color-text-secondary)]">
              冲突策略
            </p>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
              {policyOptions.map((option) => (
                <button
                  key={option.value}
                  className={cn(
                    "min-h-[76px] rounded-md border p-3 text-left transition-[background,border-color,box-shadow]",
                    conflictPolicy === option.value
                      ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] shadow-sm"
                      : "border-[var(--color-border)] bg-[var(--color-surface-raised)] hover:bg-[var(--color-surface)]",
                  )}
                  onClick={() => onPolicy(option.value)}
                  type="button"
                >
                  <span className="block text-sm font-medium">{option.label}</span>
                  <span className="mt-1 block text-xs text-[var(--color-text-secondary)]">
                    {option.helper}
                  </span>
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="flex items-center justify-between gap-3 border-t border-[var(--color-border)] bg-[var(--color-surface)] px-6 py-4">
          <p className="text-xs text-[var(--color-text-secondary)]">
            已选择 {selectedAgents.length} 个 Agent
          </p>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={onClose} disabled={busy}>
              取消
            </Button>
            <Button
              variant="primary"
              onClick={onSync}
              disabled={busy || selectedAgents.length === 0}
            >
              <RefreshCw size={14} />
              同步
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
