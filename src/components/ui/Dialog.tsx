import type { ReactNode } from "react";

/**
 * 通用对话框容器：固定遮罩 + 居中面板。
 * 收敛各视图重复的 overlay/panel 内联样式（M-F9）。
 * - 点击遮罩关闭（面板内 stopPropagation）
 * - `large` 用于大对话框（双阴影 + 限高 + 纵向布局），否则为紧凑确认框（单阴影）
 */
export function Dialog({
  maxWidth = 420,
  zIndex = 50,
  dark = false,
  large = false,
  fillHeight = false,
  onClose,
  children,
}: {
  maxWidth?: number;
  zIndex?: number;
  /** 更深的遮罩（确认框用 rgba(0,0,0,0.36)） */
  dark?: boolean;
  /** 大对话框：双阴影 + maxHeight 88vh + flex 纵向布局 */
  large?: boolean;
  /** 大对话框固定占满 88vh（与 maxHeight 同时生效），用于内容必然超高的弹窗 */
  fillHeight?: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  const panel: React.CSSProperties = large
    ? {
        height: fillHeight ? "88vh" : undefined,
        maxHeight: "88vh",
        width: "100%",
        maxWidth,
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        borderRadius: "var(--radius-lg)",
        border: "1px solid var(--border)",
        background: "var(--surface-raised)",
        boxShadow: "0 18px 55px rgba(0,0,0,0.14), 0 2px 8px rgba(0,0,0,0.06)",
      }
    : {
        width: "100%",
        maxWidth,
        borderRadius: "var(--radius-lg)",
        border: "1px solid var(--border)",
        background: "var(--surface-raised)",
        boxShadow: "0 18px 55px rgba(0,0,0,0.14)",
      };
  return (
    <div
      onClick={onClose}
      style={{
        position: "fixed",
        inset: 0,
        zIndex,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: dark ? "rgba(0, 0, 0, 0.36)" : "rgba(0, 0, 0, 0.28)",
        padding: 20,
      }}
    >
      <div onClick={(e) => e.stopPropagation()} style={panel}>
        {children}
      </div>
    </div>
  );
}