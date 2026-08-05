/** 判断是否运行在 Tauri 桌面运行时（而非纯浏览器预览）。 */
export function hasTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
