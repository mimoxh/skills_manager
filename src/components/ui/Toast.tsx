import { useEffect, useRef, useState } from "react";

export type ToastType = "success" | "error" | "info";

export interface ToastMessage {
  text: string;
  type: ToastType;
}

const ICONS: Record<ToastType, string> = {
  success: "M22 11.08V12a10 10 0 1 1-5.93-9.14 M22 4 12 14.01l-3-3",
  error: "M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z M12 9v4 M12 17h.01",
  info: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z M12 16v-4 M12 8h.01",
};

const COLORS: Record<ToastType, { bg: string; border: string; icon: string }> = {
  success: {
    bg: "var(--success-light)",
    border: "var(--success)",
    icon: "var(--success)",
  },
  error: {
    bg: "var(--danger-light)",
    border: "var(--danger)",
    icon: "var(--danger)",
  },
  info: {
    bg: "var(--accent-light)",
    border: "var(--accent)",
    icon: "var(--accent)",
  },
};

export function Toast({ message, onDismiss }: { message: ToastMessage | null; onDismiss: () => void }) {
  const [visible, setVisible] = useState(false);
  const [current, setCurrent] = useState<ToastMessage | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    if (message) {
      setCurrent(message);
      setVisible(true);
      if (timerRef.current) clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        setVisible(false);
        setTimeout(onDismiss, 200); // Wait for exit animation
      }, 4000);
    }
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [message, onDismiss]);

  if (!current) return null;

  const colors = COLORS[current.type];
  const iconPath = ICONS[current.type];

  return (
    <div
      style={{
        position: "fixed",
        bottom: 24,
        right: 24,
        zIndex: 200,
        maxWidth: 420,
        opacity: visible ? 1 : 0,
        transform: visible ? "translateY(0)" : "translateY(12px)",
        transition: "opacity 0.2s ease, transform 0.2s ease",
        pointerEvents: visible ? "auto" : "none",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "flex-start",
          gap: 12,
          padding: "14px 18px",
          borderRadius: "var(--radius-md)",
          border: `1px solid ${colors.border}`,
          background: colors.bg,
          boxShadow: "var(--shadow-lg)",
          backdropFilter: "blur(8px)",
        }}
      >
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke={colors.icon}
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          style={{ flexShrink: 0, marginTop: 1 }}
        >
          <path d={iconPath} />
        </svg>
        <p style={{ flex: 1, fontSize: 13, lineHeight: 1.5, color: "var(--text)", margin: 0 }}>
          {current.text}
        </p>
        <button
          onClick={() => {
            setVisible(false);
            setTimeout(onDismiss, 200);
          }}
          style={{
            flexShrink: 0,
            background: "none",
            border: "none",
            cursor: "pointer",
            padding: 2,
            color: "var(--text-tertiary)",
            display: "flex",
            alignItems: "center",
          }}
          title="关闭"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </div>
  );
}
