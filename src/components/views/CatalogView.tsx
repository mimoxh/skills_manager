import { useEffect, useMemo, useState } from "react";
import type {
  CatalogFilters,
  CatalogSkill,
  CatalogSort,
  CatalogSource,
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
interface CatalogViewProps {
  busy: boolean;
  startupRefreshing: boolean;
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
  onSaveSource: (source: CatalogSource) => Promise<void>;
}

export function CatalogView({
  busy,
  startupRefreshing,
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
  onSaveSource,
}: CatalogViewProps) {
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
    onSearch(query, sort, next, 1);
  }

  function toggleArray(key: keyof Pick<CatalogFilters, "sourceIds" | "agentTypes" | "installStatuses" | "contentCapabilities">, value: string) {
    const current = filters[key] as string[];
    const nextValues = current.includes(value) ? current.filter((item) => item !== value) : [...current, value];
    updateFilters({ ...filters, [key]: nextValues });
  }

  async function applySearch(nextQuery: string) {
    onQuery(nextQuery);
    await onSearch(nextQuery, sort, filters, 1);
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
              <article className="catalog-card" key={skill.id}>
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
            {total > 0 && (
              <div className="catalog-pagination">
                <button className="btn btn-secondary btn-sm" onClick={() => onPage(page - 1)} disabled={busy || page <= 1} type="button">
                  上一页
                </button>
                <span>第 {page} 页 · {firstVisible}-{lastVisible} / {total}</span>
                <button className="btn btn-secondary btn-sm" onClick={() => onPage(page + 1)} disabled={busy || !hasMore} type="button">
                  下一页
                </button>
              </div>
            )}
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
