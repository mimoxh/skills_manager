import { useRef, useState } from "react";
import { api } from "../api";
import type { ToastType } from "../components/ui/Toast";
import type {
  CatalogFilters,
  CatalogRefreshStatus,
  CatalogSafetyMode,
  CatalogSkill,
  CatalogSort,
  CatalogSource,
  ConflictPolicy,
  InstallResult,
} from "../types";

const emptyCatalogFilters: CatalogFilters = {
  sourceIds: [],
  agentTypes: [],
  installStatuses: [],
  hasDownloadData: null,
  timeWindowDays: null,
  contentCapabilities: [],
  safetyMode: "all",
};

interface Props {
  showToast: (text: string, type?: ToastType) => void;
  refreshAll: () => Promise<void>;
  /** 内置默认 catalog 源 id（后端 InitialData 下发），替代硬编码 "clawhub" */
  defaultSourceId: string;
}

export function useCatalog({ showToast, refreshAll, defaultSourceId }: Props) {
  const [catalogBusy, setCatalogBusy] = useState(false);
  const [catalogStartupRefreshing, setCatalogStartupRefreshing] = useState(false);
  const [catalogSources, setCatalogSources] = useState<CatalogSource[]>([]);
  const [catalogSkills, setCatalogSkills] = useState<CatalogSkill[]>([]);
  const [catalogTotal, setCatalogTotal] = useState(0);
  const [catalogPage, setCatalogPage] = useState(1);
  const catalogPageSize = 100;
  const [catalogHasMore, setCatalogHasMore] = useState(false);
  const [catalogRefreshStatuses, setCatalogRefreshStatuses] = useState<Record<CatalogSafetyMode, CatalogRefreshStatus | null>>({
    all: null,
    nonSuspicious: null,
  });
  const [catalogQuery, setCatalogQuery] = useState("");
  const [catalogSort, setCatalogSort] = useState<CatalogSort>("updatedDesc");
  const [catalogFilters, setCatalogFilters] = useState<CatalogFilters>(emptyCatalogFilters);

  async function searchCatalog(
    nextQuery = catalogQuery,
    nextSort = catalogSort,
    nextFilters = catalogFilters,
    nextPage = catalogPage,
  ) {
    setCatalogBusy(true);
    try {
      const [sources, result] = await Promise.all([
        api.listCatalogSources(),
        api.searchCatalogSkills(nextQuery, nextSort, nextFilters, nextPage, catalogPageSize),
      ]);
      setCatalogSources(sources);
      setCatalogSkills(result.items);
      setCatalogTotal(result.total);
      setCatalogPage(result.page);
      setCatalogHasMore(result.hasMore);
      showToast(`仓库目录显示第 ${result.page} 页 ${result.items.length} 个 skills，共 ${result.total} 个。`, "info");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setCatalogBusy(false);
    }
  }

  async function refreshCatalogSource(sourceId: string) {
    setCatalogBusy(true);
    try {
      const result = await api.refreshCatalogSource(sourceId);
      showToast(result.message, "success");
      await searchCatalog();
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setCatalogBusy(false);
    }
  }

  async function refreshCatalogStatus(safetyMode: CatalogSafetyMode = catalogFilters.safetyMode) {
    try {
      const status = await api.getCatalogRefreshStatus(defaultSourceId, safetyMode);
      setCatalogRefreshStatuses((previous) => ({ ...previous, [safetyMode]: status }));
      return status;
    } catch (error) {
      showToast(String(error), "error");
      return null;
    }
  }

  async function startCatalogRefresh(safetyMode: CatalogSafetyMode = catalogFilters.safetyMode) {
    setCatalogBusy(true);
    try {
      const status = await api.startCatalogRefresh(defaultSourceId, safetyMode);
      setCatalogRefreshStatuses((previous) => ({ ...previous, [safetyMode]: status }));
      showToast(`ClawHub 后台刷新已启动，当前已索引 ${status.fetchedCount} 个 skills。`, "info");
      return status;
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setCatalogBusy(false);
    }
  }

  async function cancelCatalogRefresh(safetyMode: CatalogSafetyMode = catalogFilters.safetyMode) {
    try {
      const status = await api.cancelCatalogRefresh(defaultSourceId, safetyMode);
      setCatalogRefreshStatuses((previous) => ({ ...previous, [safetyMode]: status }));
      showToast("已请求取消 ClawHub 后台刷新。", "info");
      return status;
    } catch (error) {
      showToast(String(error), "error");
    }
  }

  async function changeCatalogPage(nextPage: number) {
    const safePage = Math.max(1, nextPage);
    await searchCatalog(catalogQuery, catalogSort, catalogFilters, safePage);
  }

  async function refreshCatalogOnStartup() {
    setCatalogStartupRefreshing(true);
    try {
      await searchCatalog();
      const sources = await api.listCatalogSources();
      let refreshed = 0;
      for (const source of sources.filter((source) => source.enabled && source.id !== defaultSourceId)) {
        try {
          await api.refreshCatalogSource(source.id);
          refreshed += 1;
        } catch (error) {
          console.warn(`Catalog source refresh failed: ${source.id}`, error);
        }
      }
      const [nextSources, nextSkills] = await Promise.all([
        api.listCatalogSources(),
        api.searchCatalogSkills(catalogQuery, catalogSort, catalogFilters, catalogPage, catalogPageSize),
      ]);
      setCatalogSources(nextSources);
      setCatalogSkills(nextSkills.items);
      setCatalogTotal(nextSkills.total);
      setCatalogPage(nextSkills.page);
      setCatalogHasMore(nextSkills.hasMore);
      if (refreshed > 0) {
        showToast(`已后台更新 ${refreshed} 个仓库源，仓库目录显示第 ${nextSkills.page} 页 ${nextSkills.items.length} 个 skills，共 ${nextSkills.total} 个。`, "info");
      }
    } catch (error) {
      showToast(`仓库目录后台更新失败: ${String(error)}`, "error");
    } finally {
      setCatalogStartupRefreshing(false);
    }
  }

  // Catalog 懒加载：首次进入 Catalog 视图时才执行，避免应用启动即触发网络刷新与搜索
  const catalogLoadedRef = useRef(false);
  async function ensureCatalogLoaded() {
    if (catalogLoadedRef.current) return;
    catalogLoadedRef.current = true;
    await refreshCatalogOnStartup();
  }

  async function saveCatalogSource(source: CatalogSource) {
    setCatalogBusy(true);
    try {
      await api.saveCatalogSource(source);
      await searchCatalog();
      showToast(`已保存仓库源 ${source.name}。`, "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setCatalogBusy(false);
    }
  }

  async function installCatalogSkill(
    catalogSkillId: string,
    targetAgentIds: string[],
    conflictPolicy: ConflictPolicy,
  ): Promise<InstallResult[]> {
    if (!targetAgentIds.length) {
      showToast("请至少选择一个目标 Agent。", "error");
      return [];
    }
    setCatalogBusy(true);
    try {
      const results = await api.installCatalogSkill(catalogSkillId, targetAgentIds, conflictPolicy);
      await refreshAll();
      await searchCatalog();
      showToast(`已完成 ${results.length} 个安装任务。`, "success");
      return results;
    } catch (error) {
      showToast(String(error), "error");
      throw error;
    } finally {
      setCatalogBusy(false);
    }
  }

  return {
    catalogBusy,
    catalogStartupRefreshing,
    catalogSources,
    catalogSkills,
    catalogTotal,
    catalogPage,
    catalogPageSize,
    catalogHasMore,
    catalogRefreshStatuses,
    catalogQuery,
    catalogSort,
    catalogFilters,
    setCatalogQuery,
    setCatalogSort,
    setCatalogFilters,
    searchCatalog,
    changeCatalogPage,
    refreshCatalogSource,
    refreshCatalogStatus,
    startCatalogRefresh,
    cancelCatalogRefresh,
    ensureCatalogLoaded,
    saveCatalogSource,
    installCatalogSkill,
  };
}