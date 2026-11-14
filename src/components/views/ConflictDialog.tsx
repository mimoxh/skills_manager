import type { SyncConflict, SyncConflictChoice } from "../../types";
import { Dialog } from "../ui/Dialog";

interface ConflictDialogProps {
  conflicts: SyncConflict[];
  busy: boolean;
  onClose: () => void;
  onResolve: (skillId: string, choice: SyncConflictChoice) => void;
}

/** 同步冲突处理：逐 skill 选择 本地 / 远端 / 都留（远端改名）。 */
export function ConflictDialog({ conflicts, busy, onClose, onResolve }: ConflictDialogProps) {
  return (
    <Dialog maxWidth={720} large onClose={onClose}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, borderBottom: "1px solid var(--border)", padding: "20px 24px" }}>
        <div style={{ width: 40, height: 40, background: "var(--danger-light, var(--accent-light))", borderRadius: "var(--radius-sm)", display: "flex", alignItems: "center", justifyContent: "center", color: "var(--danger, var(--accent))", flexShrink: 0 }}>
          <svg className="icon" viewBox="0 0 24 24"><path d="M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" /><line x1="12" y1="9" x2="12" y2="13" /><line x1="12" y1="17" x2="12.01" y2="17" /></svg>
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <h2 style={{ fontSize: 16, fontWeight: 600, color: "var(--text)" }}>同步冲突</h2>
          <p style={{ fontSize: 12, color: "var(--text-secondary)", marginTop: 4 }}>
            共 {conflicts.length} 个 skill 在两台设备上都有改动，请选择保留哪一版
          </p>
        </div>
        <button className="btn-icon" onClick={onClose} type="button" title="关闭" style={{ width: 36, height: 36 }}>
          <svg className="icon" viewBox="0 0 24 24"><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
        </button>
      </div>

      <div style={{ flex: 1, overflow: "auto", padding: "16px 24px", display: "flex", flexDirection: "column", gap: 12 }}>
        {conflicts.length === 0 && (
          <p style={{ fontSize: 13, color: "var(--text-tertiary)", textAlign: "center", padding: "24px 0" }}>
            没有待处理冲突。
          </p>
        )}
        {conflicts.map((conflict) => (
          <div key={conflict.skillId} className="card" style={{ padding: 16 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 8 }}>
              <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text)" }}>{conflict.name}</span>
              <span className="badge" style={{ fontSize: 11 }}>
                {conflict.kind === "deleteVsModify" ? "删除 vs 修改" : "双端修改"}
              </span>
            </div>
            <div style={{ fontSize: 12, color: "var(--text-secondary)", display: "flex", flexDirection: "column", gap: 3, marginBottom: 12 }}>
              <span>本机：<code style={{ fontFamily: "var(--font-mono)" }}>{conflict.localHash ? conflict.localHash.slice(0, 12) : "已删除"}</code></span>
              <span>远端（{conflict.remoteDeviceId || "未知设备"}）：<code style={{ fontFamily: "var(--font-mono)" }}>{conflict.remoteHash ? conflict.remoteHash.slice(0, 12) : "已删除"}</code></span>
            </div>
            <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
              <button className="btn btn-secondary btn-sm" disabled={busy} type="button" onClick={() => onResolve(conflict.skillId, "local")}>
                保留本机
              </button>
              <button className="btn btn-primary btn-sm" disabled={busy || !conflict.remoteHash} type="button" onClick={() => onResolve(conflict.skillId, "remote")}>
                使用远端
              </button>
              <button className="btn btn-secondary btn-sm" disabled={busy || !conflict.remoteHash} type="button" onClick={() => onResolve(conflict.skillId, "rename")}>
                都保留（远端改名）
              </button>
            </div>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end", borderTop: "1px solid var(--border)", background: "var(--surface-raised)", padding: "16px 24px" }}>
        <button className="btn btn-secondary" onClick={onClose} disabled={busy} type="button">关闭</button>
      </div>
    </Dialog>
  );
}
