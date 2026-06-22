import { useMemo, useState } from "react";

interface UserTagEditorProps {
  availableTags?: string[];
  busy: boolean;
  onChange: (tags: string[]) => void | Promise<void>;
  tags: string[];
}

export function UserTagEditor({ availableTags = [], busy, onChange, tags }: UserTagEditorProps) {
  const [tagInput, setTagInput] = useState("");
  const [tagError, setTagError] = useState<string | null>(null);

  const suggestedTags = useMemo(() => {
    const currentTags = new Set(tags.map((tag) => tag.toLowerCase()));
    const suggestions = new Map<string, string>();
    for (const tag of availableTags) {
      const trimmed = tag.trim();
      const key = trimmed.toLowerCase();
      if (!trimmed || currentTags.has(key) || suggestions.has(key)) continue;
      suggestions.set(key, trimmed);
    }
    return [...suggestions.values()].sort((a, b) => a.localeCompare(b));
  }, [availableTags, tags]);

  async function addTagValue(rawTag: string) {
    const tag = rawTag.trim();
    if (!tag) return;
    if (Array.from(tag).length > 32) {
      setTagError("标签不能超过 32 个字符。");
      return;
    }
    if (tags.some((existing) => existing.toLowerCase() === tag.toLowerCase())) {
      setTagInput("");
      setTagError(null);
      return;
    }
    setTagInput("");
    setTagError(null);
    try {
      await onChange([...tags, tag]);
    } catch (error) {
      setTagError(String(error));
    }
  }

  async function removeTag(tag: string) {
    setTagError(null);
    try {
      await onChange(tags.filter((existing) => existing.toLowerCase() !== tag.toLowerCase()));
    } catch (error) {
      setTagError(String(error));
    }
  }

  return (
    <>
      <div className="skill-tag-editor">
        {tags.length > 0 ? (
          tags.map((tag) => (
            <button
              className="badge badge-user-tag skill-tag-remove"
              disabled={busy}
              key={tag}
              onClick={() => void removeTag(tag)}
              title={`删除标签 ${tag}`}
              type="button"
            >
              {tag}
              <span aria-hidden="true">×</span>
            </button>
          ))
        ) : (
          <span className="skill-tag-editor-empty">暂无标签</span>
        )}
      </div>
      <div className="skill-tag-editor-input">
        <input
          disabled={busy}
          maxLength={64}
          onChange={(event) => {
            setTagInput(event.target.value);
            if (tagError) setTagError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void addTagValue(tagInput);
            }
          }}
          placeholder="输入标签后按 Enter"
          type="text"
          value={tagInput}
        />
        <button className="btn btn-secondary btn-sm" disabled={busy || !tagInput.trim()} onClick={() => void addTagValue(tagInput)} type="button">
          添加
        </button>
      </div>
      {suggestedTags.length > 0 && (
        <div className="skill-tag-suggestions">
          <span className="skills-tag-filter-label">已用标签</span>
          {suggestedTags.map((tag) => (
            <button
              className="badge badge-user-tag skill-tag-filter"
              disabled={busy}
              key={tag}
              onClick={() => void addTagValue(tag)}
              type="button"
            >
              {tag}
            </button>
          ))}
        </div>
      )}
      {tagError && <p className="skill-tag-editor-error">{tagError}</p>}
    </>
  );
}
