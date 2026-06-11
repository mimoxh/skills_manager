import { useEffect, useMemo, useState } from "react";
import type {
  AgentProfile,
  CatalogFilters,
  CatalogSkill,
  CatalogSort,
  CatalogSource,
  ConflictPolicy,
  InstallResult,
} from "../../types";

const sortOptions: Array<{ value: CatalogSort; label: string }> = [
  { value: "updatedDesc", label: "最近更新" },
  { value: "downloads", label: "最多下载/安装" },
  { value: "publishedDesc", label: "最新发布" },
  { value: "source", label: "来源" },
];

const agentOptions = ["codex", "claude", "claudeCode", "cursor", "opencode", "openclaw", "unknown"];
const contentOptions = [
  { value: "scripts", label: "有 scripts" },
  { value: "references", label: "有 references" },
  { value: "assets", label: "有 assets" },
  { value: "skillMdOnly", label: "仅 SKILL.md" },
];
const policyOptions: Array<{ value: ConflictPolicy; label: string }> = [
  { value: "backupOverwrite", label: "备份覆盖" },
  { value: "skip", label: "跳过冲突" },
  { value: "rename", label: "另存副本" },
];

interface CatalogViewProps {
  agents: AgentProfile[];
  busy: boolean;
  startupRefreshing: boolean;
  sources: CatalogSource[];
  skills: CatalogSkill[];
  query: string;
  sort: CatalogSort;
  filters: CatalogFilters;
  onQuery: (query: string) => void;
  onSort: (sort: CatalogSort) => void;
  onFilters: (filters: CatalogFilters) => void;
  onSearch: (query?: string, sort?: CatalogSort, filters?: CatalogFilters) => Promise<void>;
  onRefreshSource: (sourceId: string) => Promise<void>;
  onSaveSource: (source: CatalogSource) => Promise<void>;
  onInstall: (catalogSkillId: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<InstallResult[]>;
}

export function CatalogView({
  agents,
  busy,
  startupRefreshing,
  sources,
  skills,
  query,
  sort,
  filters,
  onQuery,
  onSort,
  onFilters,
  onSearch,
  onRefreshSource,
  onSaveSource,
  onInstall,
}: CatalogViewProps) {
  const [selectedSkill, setSelectedSkill] = useState<CatalogSkill | null>(null);
  const [customOpen, setCustomOpen] = useState(false);
  const [customName, setCustomName] = useState("");
  const [customUrl, setCustomUrl] = useState("");

  useEffect(() => {
    if (startupRefreshing || sources.length || skills.length) {
      return;
    }
    onSearch(query, sort, filters);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [startupRefreshing]);

  function updateFilters(next: CatalogFilters) {
    onFilters(next);
    onSearch(query, sort, next);
  }

  function toggleArray(key: keyof Pick<CatalogFilters, "sourceIds" | "agentTypes" | "installStatuses" | "contentCapabilities">, value: string) {
    const current = filters[key] as string[];
    const nextValues = current.includes(value) ? current.filter((item) => item !== value) : [...current, value];
    updateFilters({ ...filters, [key]: nextValues });
  }

  async function applySearch(nextQuery: string) {
    onQuery(nextQuery);
    await onSearch(nextQuery, sort, filters);
  }

  async function applySort(nextSort: CatalogSort) {
    onSort(nextSort);
    await onSearch(query, nextSort, filters);
  }

  async function saveCustomSource() {
    const name = customName.trim();
    const url = customUrl.trim();
    if (!name || !url) return;
    await onSaveSource({
      id: "",
      name,
      url,
      kind: "custom",
      icon: "custom",
      enabled: true,
      lastRefreshedAt: null,
      cachePath: null,
    });
    setCustomName("");
    setCustomUrl("");
    setCustomOpen(false);
  }

  const sourceCounts = useMemo(() => {
    return skills.reduce<Record<string, number>>((acc, skill) => {
      acc[skill.sourceId] = (acc[skill.sourceId] ?? 0) + 1;
      return acc;
    }, {});
  }, [skills]);

  return (
    <>
      <div className="catalog-view">
        <div className="catalog-toolbar">
          <div className="catalog-search">
            <svg className="icon icon-sm" viewBox="0 0 24 24"><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></svg>
            <input
              value={query}
              onChange={(event) => applySearch(event.target.value)}
              placeholder="搜索 skill 名称、描述、标签或来源"
            />
          </div>
          <select className="catalog-sort" value={sort} onChange={(event) => applySort(event.target.value as CatalogSort)} disabled={busy}>
            {sortOptions.map((option) => (
              <option key={option.value} value={option.value}>{option.label}</option>
            ))}
          </select>
        </div>

        <div className="catalog-layout">
          <aside className="catalog-filters">
            <div className="catalog-filter-header">
              <div>
                <div className="card-title">筛选</div>
                <div className="card-desc">{skills.length} 个可见项目</div>
              </div>
              <button className="btn-icon" onClick={() => onSearch(query, sort, filters)} disabled={busy} title="刷新列表" type="button">
                <svg className="icon icon-sm" viewBox="0 0 24 24"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
              </button>
            </div>

            <FilterSection title="来源">
              {sources.map((source) => (
                <label className="catalog-check" key={source.id}>
                  <input
                    type="checkbox"
                    checked={filters.sourceIds.includes(source.id)}
                    onChange={() => toggleArray("sourceIds", source.id)}
                  />
                  <span>{source.name}</span>
                  <em>{sourceCounts[source.id] ?? 0}</em>
                </label>
              ))}
              <button className="btn btn-secondary btn-sm catalog-full-btn" onClick={() => setCustomOpen(true)} type="button">
                添加自定义仓库
              </button>
            </FilterSection>

            <FilterSection title="仓库刷新">
              {sources.map((source) => (
                <button className="catalog-source-refresh" key={source.id} onClick={() => onRefreshSource(source.id)} disabled={busy} type="button">
                  <span>{source.name}</span>
                  <small>{source.lastRefreshedAt ? "已刷新" : "未刷新"}</small>
                </button>
              ))}
            </FilterSection>

            <FilterSection title="Agent 兼容">
              {agentOptions.map((agent) => (
                <label className="catalog-check" key={agent}>
                  <input type="checkbox" checked={filters.agentTypes.includes(agent)} onChange={() => toggleArray("agentTypes", agent)} />
                  <span>{agent}</span>
                </label>
              ))}
            </FilterSection>

            <FilterSection title="安装状态">
              {[
                ["notInstalled", "未安装"],
                ["installed", "已安装"],
                ["updateAvailable", "有更新"],
                ["conflict", "冲突"],
              ].map(([value, label]) => (
                <label className="catalog-check" key={value}>
                  <input
                    type="checkbox"
                    checked={filters.installStatuses.includes(value as CatalogFilters["installStatuses"][number])}
                    onChange={() => toggleArray("installStatuses", value)}
                  />
                  <span>{label}</span>
                </label>
              ))}
            </FilterSection>

            <FilterSection title="热度数据">
              <select
                className="input"
                value={filters.hasDownloadData === null || filters.hasDownloadData === undefined ? "all" : filters.hasDownloadData ? "yes" : "no"}
                onChange={(event) => updateFilters({ ...filters, hasDownloadData: event.target.value === "all" ? null : event.target.value === "yes" })}
              >
                <option value="all">全部</option>
                <option value="yes">有下载/安装数据</option>
                <option value="no">无下载/安装数据</option>
              </select>
            </FilterSection>

            <FilterSection title="时间">
              <select
                className="input"
                value={filters.timeWindowDays ?? "all"}
                onChange={(event) => updateFilters({ ...filters, timeWindowDays: event.target.value === "all" ? null : Number(event.target.value) })}
              >
                <option value="all">全部</option>
                <option value="7">最近 7 天</option>
                <option value="30">最近 30 天</option>
                <option value="90">最近 90 天</option>
              </select>
            </FilterSection>

            <FilterSection title="内容能力">
              {contentOptions.map((option) => (
                <label className="catalog-check" key={option.value}>
                  <input
                    type="checkbox"
                    checked={filters.contentCapabilities.includes(option.value)}
                    onChange={() => toggleArray("contentCapabilities", option.value)}
                  />
                  <span>{option.label}</span>
                </label>
              ))}
            </FilterSection>
          </aside>

          <main className="catalog-main">
            {skills.map((skill) => (
              <button className="catalog-card" key={skill.id} onClick={() => setSelectedSkill(skill)} type="button">
                <div className="catalog-card-top">
                  <SourceIcon icon={skill.sourceIcon} />
                  <span className={`badge ${skill.installStatus === "installed" ? "badge-success" : "badge-muted"}`}>
                    {skill.installStatus === "installed" ? "已安装" : "未安装"}
                  </span>
                </div>
                <div className="catalog-card-name">{skill.name}</div>
                <div className="catalog-card-desc">{skill.description || "暂无描述"}</div>
                <div className="catalog-card-tags">
                  {skill.hasScripts && <span>scripts</span>}
                  {skill.hasReferences && <span>references</span>}
                  {skill.hasAssets && <span>assets</span>}
                </div>
                <div className="catalog-card-meta">
                  <span>{formatCatalogUsage(skill)}</span>
                  <span className="catalog-card-source">
                    <span>{skill.sourceName}</span>
                    <SourceIcon icon={skill.sourceIcon} small />
                  </span>
                </div>
              </button>
            ))}
            {!skills.length && startupRefreshing && <div className="catalog-empty" aria-hidden="true" />}
            {!skills.length && !startupRefreshing && (
              <div className="catalog-empty">
                <p>没有找到 catalog skills</p>
                <span>先刷新内置源或添加自定义仓库后再搜索</span>
              </div>
            )}
          </main>
        </div>
      </div>

      {selectedSkill && (
        <InstallCatalogDialog
          agents={agents}
          busy={busy}
          skill={selectedSkill}
          onClose={() => setSelectedSkill(null)}
          onInstall={onInstall}
        />
      )}

      {customOpen && (
        <div className="dialog-backdrop" onClick={() => setCustomOpen(false)}>
          <div className="dialog catalog-dialog" onClick={(event) => event.stopPropagation()}>
            <div className="dialog-header">
              <div>
                <div className="dialog-title">添加自定义仓库</div>
                <div className="dialog-subtitle">支持 Git 仓库地址</div>
              </div>
              <button className="btn-icon" onClick={() => setCustomOpen(false)} type="button">×</button>
            </div>
            <div className="input-group">
              <label className="input-label">名称</label>
              <input className="input" value={customName} onChange={(event) => setCustomName(event.target.value)} />
            </div>
            <div className="input-group">
              <label className="input-label">Git URL</label>
              <input className="input" value={customUrl} onChange={(event) => setCustomUrl(event.target.value)} placeholder="https://github.com/owner/repo" />
            </div>
            <div className="dialog-actions">
              <button className="btn btn-ghost" onClick={() => setCustomOpen(false)} type="button">取消</button>
              <button className="btn btn-primary" onClick={saveCustomSource} disabled={busy || !customName.trim() || !customUrl.trim()} type="button">保存</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function FilterSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="catalog-filter-section">
      <div className="catalog-filter-title">{title}</div>
      {children}
    </section>
  );
}

function SourceIcon({ icon, small = false }: { icon: string; small?: boolean }) {
  const label = icon === "clawhub" ? "C" : icon === "claude" ? "A" : icon === "codex" ? "O" : "G";
  return <span className={`catalog-source-icon ${small ? "small" : ""} ${icon}`}>{label}</span>;
}

function formatCatalogUsage(skill: CatalogSkill) {
  if (skill.downloadCount !== null && skill.downloadCount !== undefined) {
    return `下载 ${formatCompactNumber(skill.downloadCount)}`;
  }
  if (skill.installCount !== null && skill.installCount !== undefined) {
    return `安装 ${formatCompactNumber(skill.installCount)}`;
  }
  return "无下载数据";
}

function formatCompactNumber(value: number) {
  if (value >= 10000) {
    return `${(value / 10000).toFixed(value >= 100000 ? 0 : 1)}万`;
  }
  return value.toLocaleString("zh-CN");
}

function InstallCatalogDialog({
  agents,
  busy,
  skill,
  onClose,
  onInstall,
}: {
  agents: AgentProfile[];
  busy: boolean;
  skill: CatalogSkill;
  onClose: () => void;
  onInstall: (catalogSkillId: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<InstallResult[]>;
}) {
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [policy, setPolicy] = useState<ConflictPolicy>("backupOverwrite");

  async function install() {
    await onInstall(skill.id, selectedAgents, policy);
    onClose();
  }

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div className="dialog catalog-dialog" onClick={(event) => event.stopPropagation()}>
        <div className="dialog-header">
          <div>
            <div className="dialog-title">{skill.name}</div>
            <div className="dialog-subtitle">{skill.sourceName} · {skill.relativePath}</div>
          </div>
          <button className="btn-icon" onClick={onClose} type="button">×</button>
        </div>
        <div className="catalog-security-note">
          第三方 skill 默认未审核。安装前请检查 SKILL.md、scripts 和依赖。
        </div>
        <div className="catalog-agent-list">
          {agents.map((agent) => (
            <label className="agent-item" key={agent.id}>
              <input
                type="checkbox"
                checked={selectedAgents.includes(agent.id)}
                onChange={() => setSelectedAgents((prev) => prev.includes(agent.id) ? prev.filter((id) => id !== agent.id) : [...prev, agent.id])}
              />
              <div className="agent-info">
                <div className="agent-name">{agent.name}</div>
                <div className="agent-path">{agent.skillsPath}</div>
              </div>
            </label>
          ))}
        </div>
        <div className="input-group">
          <label className="input-label">冲突策略</label>
          <select className="input" value={policy} onChange={(event) => setPolicy(event.target.value as ConflictPolicy)}>
            {policyOptions.map((option) => (
              <option key={option.value} value={option.value}>{option.label}</option>
            ))}
          </select>
        </div>
        <div className="dialog-actions">
          <button className="btn btn-ghost" onClick={onClose} type="button">取消</button>
          <button className="btn btn-primary" onClick={install} disabled={busy || !selectedAgents.length} type="button">安装</button>
        </div>
      </div>
    </div>
  );
}
