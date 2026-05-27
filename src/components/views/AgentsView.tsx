import { useState } from "react";
import { Check, Bot } from "lucide-react";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "../ui/card";
import { Input } from "../ui/input";
import { cn } from "../../lib/utils";
import type { AgentProfile, GroupedSkill } from "../../types";

interface AgentsViewProps {
  agents: AgentProfile[];
  skills: GroupedSkill[];
  customAgent: AgentProfile;
  busy: boolean;
  onCustomChange: (agent: AgentProfile) => void;
  onSaveCustom: () => void;
}

export function AgentsView({ agents, skills, customAgent, busy, onCustomChange, onSaveCustom }: AgentsViewProps) {
  const [detailAgentId, setDetailAgentId] = useState<string | null>(null);

  const detailAgent = detailAgentId ? agents.find((a) => a.id === detailAgentId) : null;
  const installedSkills = detailAgent
    ? skills.filter((s) => s.installedAgentIds.includes(detailAgent.id))
    : [];
  const missingSkills = detailAgent
    ? skills.filter((s) => s.missingAgentIds.includes(detailAgent.id))
    : [];

  function handleAgentClick(agentId: string) {
    setDetailAgentId((prev) => (prev === agentId ? null : agentId));
  }

  return (
    <div className="grid h-full min-h-0 grid-cols-1 gap-5 xl:grid-cols-[minmax(0,1fr)_360px]">
      <Card className="flex flex-col min-h-0">
        <CardHeader>
          <CardTitle>Agents</CardTitle>
          <CardDescription>{agents.length} 个本地 Agent 配置</CardDescription>
        </CardHeader>
        <CardContent className="flex-1 overflow-auto px-4 pb-4">
          <div className="flex flex-col gap-2.5">
            {agents.map((agent) => {
              const isDetail = detailAgentId === agent.id;
              return (
                <button
                  key={agent.id}
                  className={cn(
                    "flex min-h-[82px] w-full items-center gap-3 rounded-md border px-3 py-3 text-left transition-[background,border-color,box-shadow]",
                    isDetail
                      ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] shadow-sm"
                      : "border-[var(--color-border)] bg-[var(--color-surface-raised)] hover:bg-[var(--color-surface)]",
                  )}
                  onClick={() => handleAgentClick(agent.id)}
                  type="button"
                >
                  <div className="flex-1 min-w-0">
                    <p className="truncate text-sm font-semibold text-[var(--color-text)]">{agent.name} <span className="font-normal text-[var(--color-text-secondary)]">({agent.type})</span></p>
                    <p className="mt-1 truncate text-xs text-[var(--color-text-secondary)]">{agent.skillsPath}</p>
                    <div className="mt-2 flex gap-1.5">
                      <Badge variant="success">
                        {skills.filter((s) => s.installedAgentIds.includes(agent.id)).length} 已有
                      </Badge>
                      {skills.filter((s) => s.missingAgentIds.includes(agent.id)).length > 0 && (
                        <Badge variant="warning">
                          {skills.filter((s) => s.missingAgentIds.includes(agent.id)).length} 缺失
                        </Badge>
                      )}
                    </div>
                  </div>
                </button>
              );
            })}
            {!agents.length && (
              <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] py-10 text-center">
                <p className="text-sm font-medium text-[var(--color-text-secondary)]">没有发现 Agent</p>
                <p className="text-xs text-[var(--color-text-tertiary)]">可以添加自定义 Agent，把普通目录纳入管理</p>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      <div className="flex min-h-0 flex-col gap-5 overflow-auto">
        {detailAgent ? (
          <Card className="flex flex-col">
            <CardHeader>
              <div className="flex items-center gap-2.5">
                <div className="flex h-10 w-10 items-center justify-center rounded-md bg-[var(--color-accent-light)] text-[var(--color-accent)]">
                  <Bot size={16} />
                </div>
                <div className="flex-1 min-w-0">
                  <CardTitle className="text-sm">{detailAgent.name}</CardTitle>
                  <CardDescription className="truncate">{detailAgent.skillsPath}</CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className="flex flex-col gap-4">
              <div>
                <p className="mb-2 text-xs font-semibold text-[var(--color-text-secondary)]">已安装 ({installedSkills.length})</p>
                <div className="flex flex-col gap-1">
                  {installedSkills.map((s) => (
                    <div
                      key={s.title}
                      className="flex min-h-9 items-center gap-2 rounded-md border border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-[13px]"
                    >
                      <span className="flex-1 truncate">{s.title}</span>
                      <Badge variant="secondary">{s.bestCopy.version ? `v${s.bestCopy.version}` : "-"}</Badge>
                    </div>
                  ))}
                  {!installedSkills.length && (
                    <p className="text-xs text-[var(--color-text-tertiary)] py-2">暂无已安装 skills</p>
                  )}
                </div>
              </div>
              {missingSkills.length > 0 && (
                <div>
                  <p className="mb-2 text-xs font-semibold text-[var(--color-text-secondary)]">缺失 ({missingSkills.length})</p>
                  <div className="flex flex-col gap-1">
                    {missingSkills.map((s) => (
                      <div
                        key={s.title}
                        className="flex min-h-9 items-center gap-2 rounded-md border border-dashed border-[var(--color-border)] bg-[var(--color-surface)] px-2.5 py-1.5 text-[13px] text-[var(--color-text-secondary)]"
                      >
                        <span className="flex-1 truncate">{s.title}</span>
                        <Badge variant="warning">缺失</Badge>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        ) : (
          <Card className="flex flex-col">
            <CardHeader>
              <CardTitle>自定义 Agent</CardTitle>
              <CardDescription>添加一个本地目录作为 Agent 配置</CardDescription>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
              <div className="flex flex-col gap-1.5">
                <label className="text-xs font-medium text-[var(--color-text-secondary)]">名称</label>
                <Input
                  value={customAgent.name}
                  onChange={(e) => onCustomChange({ ...customAgent, name: e.target.value })}
                  placeholder="例如 My Agent"
                />
              </div>
              <div className="flex flex-col gap-1.5">
                <label className="text-xs font-medium text-[var(--color-text-secondary)]">Skills 安装目录</label>
                <Input
                  value={customAgent.skillsPath}
                  onChange={(e) => onCustomChange({ ...customAgent, skillsPath: e.target.value })}
                  placeholder="C:\Users\you\.agent\skills"
                />
              </div>
              <Button variant="primary" onClick={onSaveCustom} disabled={busy} className="mt-1">
                <Check size={14} />
                添加 Agent
              </Button>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
