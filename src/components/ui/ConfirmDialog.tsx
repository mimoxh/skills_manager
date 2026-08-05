import { Dialog } from "./Dialog";

/** 通用确认对话框：遮罩 + 卡片 + 取消/确认按钮。 */
export function ConfirmDialog({
  title, message, confirmLabel, busy,
  onClose, onConfirm,
}: {
  title: string;
  message: string;
  confirmLabel: string;
  busy: boolean;
  onClose: () => void;
  onConfirm: () => void;
}) {
  return (
    <Dialog maxWidth={420} zIndex={60} dark onClose={onClose}>
      <div style={{ padding: "20px 24px" }}>
        <h3 style={{ fontSize: 15, fontWeight: 600, color: "var(--text)", marginBottom: 8 }}>{title}</h3>
        <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6 }}>{message}</p>
      </div>
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, padding: "14px 24px", borderTop: "1px solid var(--border)" }}>
        <button className="btn btn-secondary" onClick={onClose} disabled={busy} type="button">取消</button>
        <button className="btn btn-danger" onClick={onConfirm} disabled={busy} type="button">{confirmLabel}</button>
      </div>
    </Dialog>
  );
}