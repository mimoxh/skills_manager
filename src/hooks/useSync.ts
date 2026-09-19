import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import type { ToastType } from "../components/ui/Toast";
import type { SyncConfig, SyncConflict, SyncConflictChoice, SyncStatus } from "../types";

const INSTALL_SCOPE_KEY = "skills-manager.installToHub";

/** 新安装默认范围：true = 同步到中枢，false = 仅本机。localStorage 持久化。 */
export function useInstallScope() {
  const [installToHub, setInstallToHubState] = useState<boolean>(() => {
    const stored = localStorage.getItem(INSTALL_SCOPE_KEY);
    return stored === null ? true : stored === "true";
  });
  const setInstallToHub = useCallback((value: boolean) => {
    setInstallToHubState(value);
    localStorage.setItem(INSTALL_SCOPE_KEY, String(value));
  }, []);
  return { installToHub, setInstallToHub };
}

interface Props {
  showToast: (text: string, type?: ToastType) => void;
}

export function useSync({ showToast }: Props) {
  const [syncConfig, setSyncConfig] = useState<SyncConfig | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [syncConflicts, setSyncConflicts] = useState<SyncConflict[]>([]);
  const [syncBusy, setSyncBusy] = useState(false);

  async function refreshSync() {
    try {
      const [config, status, conflicts] = await Promise.all([
        api.syncGetConfig(),
        api.syncStatus(),
        api.syncListConflicts(),
      ]);
      setSyncConfig(config);
      setSyncStatus(status);
      setSyncConflicts(conflicts);
    } catch (error) {
      showToast(String(error), "error");
    }
  }

  useEffect(() => {
    void refreshSync();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function saveSyncConfig(config: SyncConfig, secretAccessKey?: string, encryptPassword?: string) {
    setSyncBusy(true);
    try {
      const saved = await api.syncSetConfig(config, secretAccessKey, encryptPassword);
      setSyncConfig(saved);
      await refreshSync();
      showToast("已保存同步配置。", "success");
    } catch (error) {
      showToast(String(error), "error");
      throw error;
    } finally {
      setSyncBusy(false);
    }
  }

  async function testSyncConnection(config: SyncConfig) {
    setSyncBusy(true);
    try {
      const message = await api.syncTestConnection(config);
      showToast(message, "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setSyncBusy(false);
    }
  }

  async function syncNow() {
    setSyncBusy(true);
    try {
      const status = await api.syncNow();
      setSyncStatus(status);
      await refreshSync();
      showToast(
        status.pendingConflicts > 0
          ? `同步完成，有 ${status.pendingConflicts} 个冲突待处理。`
          : "同步完成。",
        status.pendingConflicts > 0 ? "info" : "success",
      );
    } catch (error) {
      showToast(String(error), "error");
      await refreshSync();
    } finally {
      setSyncBusy(false);
    }
  }

  async function runSyncGc() {
    setSyncBusy(true);
    try {
      const removed = await api.syncGc();
      showToast(removed > 0 ? `已清理 ${removed} 个未引用对象。` : "没有需要清理的对象。", "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setSyncBusy(false);
    }
  }

  async function resolveSyncConflict(skillId: string, choice: SyncConflictChoice) {
    setSyncBusy(true);
    try {
      const status = await api.syncResolveConflict(skillId, choice);
      setSyncStatus(status);
      await refreshSync();
      showToast("已处理冲突。", "success");
    } catch (error) {
      showToast(String(error), "error");
    } finally {
      setSyncBusy(false);
    }
  }

  return {
    syncConfig,
    syncStatus,
    syncConflicts,
    syncBusy,
    refreshSync,
    saveSyncConfig,
    testSyncConnection,
    syncNow,
    resolveSyncConflict,
    runSyncGc,
  };
}
