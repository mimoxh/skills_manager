import { useEffect, useMemo, useState } from "react";
import type {
  AgentProfile,
  CatalogFilters,
  CatalogRefreshStatus,
  CatalogSafetyMode,
  CatalogSkill,
  CatalogSort,
  CatalogSource,
  ConflictPolicy,
  GroupedSkill,
  InstallResult,
} from "../../types";
import { SkillInstallDialog } from "./SkillInstallDialog";

const sortOptions: Array<{ value: CatalogSort; label: string }> = [
  { value: "updatedDesc", label: "最近更新" },
  { value: "downloads", label: "最多下载/安装" },
  { value: "publishedDesc", label: "最新发布" },
  { value: "source", label: "来源" },
];

const contentOptions = [
  { value: "scripts", label: "有 scripts" },
  { value: "references", label: "有 references" },
  { value: "assets", label: "有 assets" },
  { value: "skillMdOnly", label: "仅 SKILL.md" },
];
interface CatalogViewProps {
  busy: boolean;
  agents: AgentProfile[];
  localSkills: GroupedSkill[];
  startupRefreshing: boolean;
  refreshStatuses: Record<CatalogSafetyMode, CatalogRefreshStatus | null>;
  sources: CatalogSource[];
  skills: CatalogSkill[];
  total: number;
  page: number;
  pageSize: number;
  hasMore: boolean;
  query: string;
  sort: CatalogSort;
  filters: CatalogFilters;
  onQuery: (query: string) => void;
  onSort: (sort: CatalogSort) => void;
  onFilters: (filters: CatalogFilters) => void;
  onSearch: (query?: string, sort?: CatalogSort, filters?: CatalogFilters, page?: number) => Promise<void>;
  onPage: (page: number) => Promise<void>;
  onRefreshSource: (sourceId: string) => Promise<void>;
  onRefreshStatus: (safetyMode?: CatalogSafetyMode) => Promise<CatalogRefreshStatus | null>;
  onStartRefresh: (safetyMode?: CatalogSafetyMode) => Promise<CatalogRefreshStatus>;
  onCancelRefresh: (safetyMode?: CatalogSafetyMode) => Promise<CatalogRefreshStatus>;
  onSaveSource: (source: CatalogSource) => Promise<void>;
  onInstallSkill: (catalogSkillId: string, targetAgentIds: string[], conflictPolicy: ConflictPolicy) => Promise<InstallResult[]>;
  onUninstallSkill: (skillId: string, agentIds: string[]) => Promise<void>;
}

export function CatalogView({
  busy,
  agents,
  localSkills,
  startupRefreshing,
  refreshStatuses,
  sources,
  skills,
  total,
  page,
  pageSize,
  hasMore,
  query,
  sort,
  filters,
  onQuery,
  onSort,
  onFilters,
  onSearch,
  onPage,
  onRefreshSource,
  onRefreshStatus,
  onStartRefresh,
  onCancelRefresh,
  onSaveSource,
  onInstallSkill,
  onUninstallSkill,
}: CatalogViewProps) {
  const [customOpen, setCustomOpen] = useState(false);
  const [customName, setCustomName] = useState("");
  const [customUrl, setCustomUrl] = useState("");
  const [selectedSkill, setSelectedSkill] = useState<CatalogSkill | null>(null);
  const [selectedAgents, setSelectedAgents] = useState<string[]>([]);
  const [conflictPolicy, setConflictPolicy] = useState<ConflictPolicy>("backupOverwrite");
  const activeRefreshStatus = refreshStatuses[filters.safetyMode];
  const localSkillLookup = useMemo(() => {
    const exact = new Map<string, GroupedSkill>();
    const loose = new Map<string, GroupedSkill>();
    const slugs = new Map<string, GroupedSkill>();

    for (const skill of localSkills) {
      const titleKey = normalizeSkillKey(skill.title);
      const looseTitleKey = normalizeLooseSkillKey(skill.title);
      if (titleKey && !exact.has(titleKey)) {
        exact.set(titleKey, skill);
      }
      if (looseTitleKey && !loose.has(looseTitleKey)) {
        loose.set(looseTitleKey, skill);
      }
      for (const copy of skill.copies) {
        const slugKey = normalizeSkillKey(lastPathSegment(copy.skillPath));
        const looseSlugKey = normalizeLooseSkillKey(slugKey);
        if (slugKey && !slugs.has(slugKey)) {
          slugs.set(slugKey, skill);
        }
        if (looseSlugKey && !slugs.has(looseSlugKey)) {
          slugs.set(looseSlugKey, skill);
        }
      }
    }

    return { exact, loose, slugs };
  }, [localSkills]);

  const selectedLocalSkill = useMemo(() => {
    return selectedSkill ? findLocalSkill(selectedSkill, localSkillLookup) : null;
  }, [selectedSkill, localSkillLookup]);

  useEffect(() => {
    if (startupRefreshing || sources.length || skills.length) {
      return;
    }
    onSearch(query, sort, filters);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [startupRefreshing]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      onSearch(query, sort, filters, 1);
    }, 300);
    return () => window.clearTimeout(handle);
  }, [query, sort, filters]);

  useEffect(() => {
    if (!activeRefreshStatus?.isRunning) return;
    const handle = window.setInterval(async () => {
      const status = await onRefreshStatus(filters.safetyMode);
      if (status && !status.isRunning) {
        await onSearch(query, sort, filters, page);
      }
    }, 1500);
    return () => window.clearInterval(handle);
  }, [activeRefreshStatus?.isRunning, filters.safetyMode, query, sort, page]);

  function updateFilters(next: CatalogFilters) {
    onFilters(next);
    onSearch(query, sort, next, 1);
  }

  function toggleArray(key: keyof Pick<CatalogFilters, "sourceIds" | "installStatuses" | "contentCapabilities">, value: string) {
    const current = filters[key] as string[];
    const nextValues = current.includes(value) ? current.filter((item) => item !== value) : [...current, value];
    updateFilters({ ...filters, [key]: nextValues });
  }

  async function applySearch(nextQuery: string) {
    onQuery(nextQuery);
  }

  async function applySort(nextSort: CatalogSort) {
    onSort(nextSort);
    await onSearch(query, nextSort, filters, 1);
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

  function openSkill(skill: CatalogSkill) {
    const localSkill = findLocalSkill(skill, localSkillLookup);
    setSelectedSkill(skill);
    setSelectedAgents(localSkill?.installedAgentIds ?? []);
    setConflictPolicy("backupOverwrite");
  }

  function toggleAgent(agentId: string) {
    setSelectedAgents((previous) =>
      previous.includes(agentId) ? previous.filter((id) => id !== agentId) : [...previous, agentId],
    );
  }

  async function installSelectedSkill() {
    if (!selectedSkill) return;
    const deselectedIds = selectedLocalSkill
      ? selectedLocalSkill.installedAgentIds.filter((id) => !selectedAgents.includes(id))
      : [];
    if (deselectedIds.length > 0 && selectedLocalSkill) {
      await onUninstallSkill(selectedLocalSkill.title, deselectedIds);
    }
    if (selectedAgents.length > 0) {
      await onInstallSkill(selectedSkill.id, selectedAgents, conflictPolicy);
    }
    setSelectedSkill(null);
    setSelectedAgents([]);
  }

  const sourceCounts = useMemo(() => {
    return skills.reduce<Record<string, number>>((acc, skill) => {
      acc[skill.sourceId] = (acc[skill.sourceId] ?? 0) + 1;
      return acc;
    }, {});
  }, [skills]);

  const firstVisible = total === 0 ? 0 : (page - 1) * pageSize + 1;
  const lastVisible = total === 0 ? 0 : Math.min((page - 1) * pageSize + skills.length, total);

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
                <div className="card-desc">{firstVisible}-{lastVisible} / {total} 个项目</div>
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

            <FilterSection title="ClawHub 安全">
              {[
                ["all", "全部"],
                ["nonSuspicious", "非 suspicious"],
              ].map(([value, label]) => (
                <label className="catalog-check" key={value}>
                  <input
                    type="radio"
                    name="catalog-safety"
                    checked={filters.safetyMode === value}
                    onChange={() => updateFilters({ ...filters, safetyMode: value as CatalogSafetyMode })}
                  />
                  <span>{label}</span>
                </label>
              ))}
            </FilterSection>

            <FilterSection title="仓库刷新">
              <div style={{ display: "grid", gap: 8, marginBottom: 10 }}>
                <div style={{ padding: "9px 10px", border: "1px solid var(--border)", borderRadius: "var(--radius-sm)", background: "var(--surface-raised)", fontSize: 12, color: "var(--text-secondary)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
                    <span>ClawHub {filters.safetyMode === "nonSuspicious" ? "安全索引" : "全量索引"}</span>
                    <strong style={{ color: "var(--text)" }}>{formatCompactNumber(activeRefreshStatus?.fetchedCount ?? 0)}</strong>
                  </div>
                  <div style={{ marginTop: 4, color: activeRefreshStatus?.lastError ? "var(--warning)" : "var(--text-tertiary)" }}>
                    {activeRefreshStatus?.isRunning
                      ? "后台刷新中"
                      : activeRefreshStatus?.isComplete
                        ? "刷新完成"
                        : activeRefreshStatus?.lastError || "尚未完成全量刷新"}
                  </div>
                </div>
                {activeRefreshStatus?.isRunning ? (
                  <button className="btn btn-secondary btn-sm catalog-full-btn" onClick={() => onCancelRefresh(filters.safetyMode)} type="button">
                    取消 ClawHub 刷新
                  </button>
                ) : (
                  <button className="btn btn-primary btn-sm catalog-full-btn" onClick={() => onStartRefresh(filters.safetyMode)} disabled={busy} type="button">
                    {activeRefreshStatus?.fetchedCount ? "继续 ClawHub 刷新" : "后台刷新 ClawHub"}
                  </button>
                )}
              </div>
              {sources.map((source) => (
                <button
                  className="catalog-source-refresh"
                  key={source.id}
                  onClick={() => source.id === "clawhub" ? onStartRefresh(filters.safetyMode) : onRefreshSource(source.id)}
                  disabled={busy || (source.id === "clawhub" && activeRefreshStatus?.isRunning)}
                  type="button"
                >
                  <span>{source.name}</span>
                  <small>{source.id === "clawhub" ? (activeRefreshStatus?.isComplete ? "已索引" : "可刷新") : source.lastRefreshedAt ? "已刷新" : "未刷新"}</small>
                </button>
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
            <div className="catalog-card-grid">
              {skills.map((skill) => (
                <article className="catalog-card" key={skill.id} onClick={() => openSkill(skill)} role="button" tabIndex={0}>
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
                </article>
              ))}
              {!skills.length && startupRefreshing && <div className="catalog-empty" aria-hidden="true" />}
              {!skills.length && !startupRefreshing && (
                <div className="catalog-empty">
                  <p>没有找到 catalog skills</p>
                  <span>先刷新内置源或添加自定义仓库后再搜索</span>
                </div>
              )}
            </div>
            <div className="catalog-footer">
              <div className="catalog-footer-summary">
                {total > 0 ? `${firstVisible}-${lastVisible} / ${total} 个项目` : "0 个项目"}
              </div>
              {total > 0 && (
                <div className="catalog-pagination">
                  <button className="btn btn-secondary btn-sm" onClick={() => onPage(page - 1)} disabled={busy || page <= 1} type="button">
                    上一页
                  </button>
                  <span>第 {page} 页</span>
                  <button className="btn btn-secondary btn-sm" onClick={() => onPage(page + 1)} disabled={busy || !hasMore} type="button">
                    下一页
                  </button>
                </div>
              )}
              {total === 0 && (
                <div className="catalog-pagination muted">
                  <span>暂无可分页项目</span>
                </div>
              )}
            </div>
          </main>
        </div>
      </div>

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

      {selectedSkill && (
        <SkillInstallDialog
          allowNoTargets={Boolean(selectedLocalSkill)}
          agents={agents}
          busy={busy}
          conflictPolicy={conflictPolicy}
          description={selectedSkill.description}
          installedAgentIds={selectedLocalSkill?.installedAgentIds ?? []}
          metadata={[
            { label: "来源", value: selectedSkill.sourceName },
            { label: "仓库路径", value: selectedSkill.relativePath || selectedSkill.sourcePath },
            { label: "兼容 Agent", value: selectedSkill.supportedAgents.length ? selectedSkill.supportedAgents.join(", ") : "未声明" },
            { label: "已安装 Agent", value: selectedLocalSkill ? selectedLocalSkill.installedAgentIds.length : 0 },
            { label: "本地匹配", value: selectedLocalSkill?.title },
            { label: "发布时间", value: selectedSkill.publishedAt },
            { label: "更新时间", value: selectedSkill.updatedAt },
            { label: "下载", value: selectedSkill.downloadCount },
            { label: "安装", value: selectedSkill.installCount },
          ]}
          primaryLabel={catalogPrimaryLabel(selectedAgents, selectedLocalSkill)}
          selectedAgentIds={selectedAgents}
          sourceLabel={selectedSkill.sourceName}
          tags={[
            ...selectedSkill.tags,
            ...(selectedSkill.hasScripts ? ["scripts"] : []),
            ...(selectedSkill.hasReferences ? ["references"] : []),
            ...(selectedSkill.hasAssets ? ["assets"] : []),
            ...(selectedSkill.hasSkillMd ? ["SKILL.md"] : []),
          ]}
          title={selectedSkill.name}
          onClose={() => setSelectedSkill(null)}
          onConfirm={installSelectedSkill}
          onPolicy={setConflictPolicy}
          onToggleAgent={toggleAgent}
        />
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

function normalizeSkillKey(value?: string | null) {
  return (value ?? "").trim().toLowerCase();
}

function normalizeLooseSkillKey(value?: string | null) {
  return normalizeSkillKey(value).replace(/[\s_\-./\\:]+/g, "");
}

function lastPathSegment(value?: string | null) {
  const clean = (value ?? "")
    .trim()
    .replace(/^clawhub:\/\//i, "")
    .split(/[?#]/)[0];
  return clean.split(/[\\/]/).filter(Boolean).pop() ?? "";
}

function catalogSkillCandidates(skill: CatalogSkill) {
  return [
    skill.name,
    skill.relativePath,
    skill.sourcePath,
    lastPathSegment(skill.relativePath),
    lastPathSegment(skill.sourcePath),
  ];
}

function findLocalSkill(
  skill: CatalogSkill,
  lookup: { exact: Map<string, GroupedSkill>; loose: Map<string, GroupedSkill>; slugs: Map<string, GroupedSkill> },
) {
  if (skill.sourceId === "clawhub") {
    const slug = clawhubCatalogSlug(skill);
    if (slug) {
      return lookup.slugs.get(normalizeSkillKey(slug))
        ?? lookup.slugs.get(normalizeLooseSkillKey(slug))
        ?? null;
    }
  }

  const exact = lookup.exact.get(normalizeSkillKey(skill.name));
  if (exact) return exact;
  for (const candidate of catalogSkillCandidates(skill)) {
    const loose = lookup.loose.get(normalizeLooseSkillKey(candidate));
    if (loose) return loose;
  }
  return null;
}

function clawhubCatalogSlug(skill: CatalogSkill) {
  if (skill.sourcePath.toLowerCase().startsWith("clawhub://")) {
    return skill.sourcePath.slice("clawhub://".length).trim();
  }
  return skill.relativePath.trim() || lastPathSegment(skill.sourcePath);
}

function catalogPrimaryLabel(selectedAgentIds: string[], localSkill: GroupedSkill | null) {
  if (!localSkill) return "安装到 Agents";
  if (selectedAgentIds.length === 0) return "全部删除";
  if (selectedAgentIds.length < localSkill.installedAgentIds.length) return "同步并清理";
  return "安装/更新";
}
