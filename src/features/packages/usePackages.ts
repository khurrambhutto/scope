import { useCallback, useEffect, useMemo, useState } from "react";
import {
  scanPackages,
  getCachedScan,
  searchPackages,
} from "../../shared/api/packages";
import type {
  CachedScan,
  PackageSource,
  AppKind,
} from "../../shared/types/package";

export type SourceFilter = PackageSource | "all";
export type KindFilter = AppKind | "all";
export type ViewMode = "uninstall" | "updates";
export type SortMode = "default" | "largest";

interface UsePackagesState {
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  lastScan: CachedScan | null;
  query: string;
  sourceFilter: SourceFilter;
  kindFilter: KindFilter;
  viewMode: ViewMode;
  sortMode: SortMode;
}

export function usePackages() {
  const [state, setState] = useState<UsePackagesState>({
    loading: true,
    refreshing: false,
    error: null,
    lastScan: null,
    query: "",
    sourceFilter: "all",
    kindFilter: "all",
    viewMode: "uninstall",
    sortMode: "default",
  });

  const refresh = useCallback(async () => {
    setState((s) => ({ ...s, refreshing: true, error: null }));
    try {
      const cached = await scanPackages();
      setState((s) => {
        const next = { ...s, loading: false, refreshing: false, lastScan: cached };
        return next;
      });
    } catch (e) {
      setState((s) => ({
        ...s,
        loading: false,
        refreshing: false,
        error: String(e),
      }));
    }
  }, []);

  // Initial load: reuse a cached scan if present, else scan fresh.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cached = await getCachedScan();
        if (cancelled) return;
        if (cached) {
          setState((s) => ({ ...s, loading: false, lastScan: cached }));
        } else {
          await refresh();
        }
      } catch (e) {
        if (!cancelled) {
          setState((s) => ({ ...s, loading: false, error: String(e) }));
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const setQuery = useCallback(
    (q: string) => {
      setState((s) => ({ ...s, query: q }));
    },
    []
  );

  const setSourceFilter = useCallback(
    (src: SourceFilter) => {
      setState((s) => ({ ...s, sourceFilter: src }));
    },
    []
  );

  const setKindFilter = useCallback(
    (k: KindFilter) => {
      setState((s) => ({ ...s, kindFilter: k }));
    },
    []
  );

  const setViewMode = useCallback(
    (v: ViewMode) => {
      setState((s) => ({ ...s, viewMode: v }));
    },
    []
  );

  const toggleSortBySize = useCallback(() => {
    setState((s) => ({
      ...s,
      sortMode: s.sortMode === "largest" ? "default" : "largest",
    }));
  }, []);

  // The visible list is derived from one consistent snapshot. Keeping it out
  // of state prevents refreshes and filter changes from racing with a stale
  // closure that still holds the previous render's filter values.
  const packages = useMemo(() => {
    const scan = state.lastScan;
    if (!scan) return [];

    const q = state.query.trim().toLowerCase();
    const filtered = scan.packages.filter((p) => {
      if (state.sourceFilter !== "all" && p.source !== state.sourceFilter) return false;
      if (state.kindFilter !== "all" && p.app_kind !== state.kindFilter) return false;
      if (state.viewMode === "updates" && (p.source === "appimage" || !p.has_update)) return false;
      if (!q) return true;

      return [
        p.name,
        p.display_name ?? "",
        p.description ?? "",
        p.package_id,
        p.install_scope ?? "",
        p.categories ?? "",
        p.version,
      ]
        .join(" ")
        .toLowerCase()
        .includes(q);
    });

    if (state.sortMode === "largest") {
      return filtered.sort((a, b) => b.size_bytes - a.size_bytes);
    }
    return filtered;
  }, [
    state.lastScan,
    state.query,
    state.sourceFilter,
    state.kindFilter,
    state.viewMode,
    state.sortMode,
  ]);

  // Also expose the server-side search for parity; not used by the default UI
  // flow but available for future "search anywhere" affordances.
  const serverSearch = useCallback(
    async (q: string, src: PackageSource | undefined, kind: AppKind | undefined) => {
      return searchPackages(q || undefined, src, kind);
    },
    []
  );

  const updatesCount =
    state.lastScan?.packages.filter((p) => p.has_update && p.source !== "appimage").length ?? 0;

  return {
    ...state,
    packages,
    updatesCount,
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
    setViewMode,
    toggleSortBySize,
    serverSearch,
  };
}
