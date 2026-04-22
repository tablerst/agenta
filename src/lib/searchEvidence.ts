const HIGHLIGHT_STYLE =
  "background: color-mix(in srgb, var(--accent-color) 18%, transparent); color: var(--text-main); padding: 0 0.1rem; border-radius: 0.2rem;";

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function tokenizeQuery(query: string | null | undefined): string[] {
  if (!query) {
    return [];
  }

  const tokens = Array.from(
    query.matchAll(/"([^"]+)"|(\S+)/g),
    (match) => (match[1] ?? match[2] ?? "").trim(),
  )
    .filter((value) => value.length > 0)
    .sort((left, right) => right.length - left.length);

  return [...new Set(tokens)];
}

export function localizeEvidenceSource(
  source: string | null | undefined,
  t: (key: string) => string,
): string {
  if (!source) {
    return "";
  }
  const key = `search.evidenceField.${source}`;
  const translated = t(key);
  return translated === key ? source : translated;
}

export function renderHighlightedEvidence(
  snippet: string | null | undefined,
  query: string | null | undefined,
): string {
  if (!snippet) {
    return "";
  }

  const tokens = tokenizeQuery(query);
  if (tokens.length === 0) {
    return escapeHtml(snippet);
  }

  const pattern = new RegExp(tokens.map(escapeRegExp).join("|"), "gi");
  let output = "";
  let lastIndex = 0;

  for (const match of snippet.matchAll(pattern)) {
    const index = match.index ?? 0;
    output += escapeHtml(snippet.slice(lastIndex, index));
    output += `<mark style="${HIGHLIGHT_STYLE}">${escapeHtml(match[0])}</mark>`;
    lastIndex = index + match[0].length;
  }

  output += escapeHtml(snippet.slice(lastIndex));
  return output;
}
