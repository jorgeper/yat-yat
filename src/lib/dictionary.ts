// Personal-dictionary editing logic (SPEC7 FR-D3) — pure functions behind
// the Settings → Cleanup table. Mirrors the Rust caps in cleanup.rs.

import type { DictionaryEntry } from "../ipc/types";

export const MAX_DICTIONARY_ENTRIES = 200;
export const MAX_DICTIONARY_FIELD_CHARS = 100;

const clip = (s: string) => s.slice(0, MAX_DICTIONARY_FIELD_CHARS);

/** Append a row; a blank "heard" phrase is never saved (FR-D3). */
export function addEntry(list: DictionaryEntry[], from: string, to: string): DictionaryEntry[] {
  const heard = clip(from.trim());
  if (!heard || list.length >= MAX_DICTIONARY_ENTRIES) return list;
  return [...list, { from: heard, to: clip(to.trim()) }];
}

/** Commit an edit; an emptied "heard" phrase leaves the row unchanged. */
export function updateEntry(
  list: DictionaryEntry[],
  index: number,
  patch: Partial<DictionaryEntry>,
): DictionaryEntry[] {
  const current = list[index];
  if (!current) return list;
  const next = {
    from: clip((patch.from ?? current.from).trim()),
    to: clip((patch.to ?? current.to).trim()),
  };
  if (!next.from) return list;
  return list.map((entry, i) => (i === index ? next : entry));
}

export function removeEntry(list: DictionaryEntry[], index: number): DictionaryEntry[] {
  return list.filter((_, i) => i !== index);
}
