import { useCallback, useRef, useState } from "react";
import type { ToastType } from "../components/ui/Toast";

export function useToast() {
  const toastIdRef = useRef(0);
  const [toast, setToast] = useState<{ id: number; text: string; type: ToastType } | null>(null);

  const showToast = useCallback((text: string, type: ToastType = "info") => {
    toastIdRef.current += 1;
    setToast({ id: toastIdRef.current, text, type });
  }, []);

  const dismissToast = useCallback(() => {
    setToast(null);
  }, []);

  return { toast, showToast, dismissToast };
}