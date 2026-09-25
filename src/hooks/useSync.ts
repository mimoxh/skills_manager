import { useEffect, useState } from "react";
import { api } from "../api";
import type { ToastType } from "../components/ui/Toast";
import type { PendingHubSkill, SyncConfig, SyncConflict, SyncConflictChoice, SyncStatus } from "../types";

interface Props {
  showToast: (text: string, type?: ToastType) => void;
}

export function useSync({ showToast }: Props) {
  const [syncConfig, setSyncConfig] = useState<SyncConfig | null>(null);
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null);
  const [syncConflicts, setSyncConflicts] = useState<SyncConflict[]>([]);
  const [syncBusy, setSyncBusy] = useState(false);
  const [pendingHubSkills, setPendingHubSkills] = useState<PendingHubSkill[]>([]);

  async function refreshPendingHubSkills() {
    const pending = await api.listPendingHubSkills();
    setPendingHubSkills(pending);
    const unread = pending.filter((skill) => !skill.notified);
    if (unread.length > 0) {
      showToast(unread[0].message ?? `已同步 ${unread.length} 个新 Skill 到中枢，可选择要分发的 Agent。`, "info");
      await api.acknowledgePendingHubSkills(unread.map((skill) => skill.skillKey));
      setPendingHubSkills(pending.map((skill) => ({ ...skill, notified: true })));
    }
  }

  useEffect(() => {
    void refreshPendingHubSkills();
    const timer = setInterval(() => { void refreshPendingHubSkills(); }, 15000);
    return () => clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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
      await refreshPendingHubSkills();
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
      await refreshPendingHubSkills();
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
    pendingHubSkills,
    refreshPendingHubSkills,
  };
}
