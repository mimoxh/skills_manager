import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import type { KeyboardEvent } from "react";
import type { ImportSkillFile } from "../types";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** 将 File 转为后端导入所需结构（相对路径 + 字节数组）。App 上传与文件夹导入共用。 */
export async function fileToUpload(file: File, relativePath?: string): Promise<ImportSkillFile> {
  return {
    relativePath: relativePath || file.webkitRelativePath || file.name,
    bytes: Array.from(new Uint8Array(await file.arrayBuffer())),
  };
}

/** 让 role="button" 的可聚焦元素支持 Enter/空格 激活（键盘可达性） */
export function handleCardActivation(event: KeyboardEvent, onActivate: () => void) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    onActivate();
  }
}

/** 标签筛选匹配：__untagged__ 表示仅无标签项；其余按小写全匹配 */
export function matchesTags(itemTags: string[] | undefined, selectedTags: string[]): boolean {
  if (!selectedTags.length) return true;
  if (selectedTags.includes("__untagged__")) {
    return !(itemTags ?? []).length;
  }
  const tags = new Set((itemTags ?? []).map((tag) => tag.toLowerCase()));
  return selectedTags.every((tag) => tags.has(tag.toLowerCase()));
}
