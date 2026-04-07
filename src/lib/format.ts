export function formatDateTime(value: string | null | undefined): string {
  if (!value) {
    return "N/A";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function prettyJson(value: unknown): string {
  return JSON.stringify(value ?? {}, null, 2);
}

export function asArray<T>(value: T[] | null | undefined): T[] {
  return Array.isArray(value) ? value : [];
}

export function coerceString(value: string | null | undefined): string {
  return value ?? "";
}
