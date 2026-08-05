import { useEffect, useRef, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import type { Operation } from "../../shared/types/operations";
import { PackageList } from "./PackageList";
import { PackageFilters } from "./PackageFilters";
import { PackageDetail } from "./PackageDetail";
import { ConfirmOperationDialog } from "../operations/ConfirmOperationDialog";
import { useOperationTasks } from "../operations/tasks";
import { usePackages } from "./usePackages";

export function PackageScreen() {
  const {
    loading,
    refreshing,
    error,
    packages,
    lastScan,
    updatesCount,
    query,
    sourceFilter,
    kindFilter,
    viewMode,
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
    setViewMode,
    sortMode,
    toggleSortBySize,
  } = usePackages();
  const { tasks, busyByKey } = useOperationTasks();

  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [confirmTarget, setConfirmTarget] = useState<{
    pkg: InstalledPackage;
    kind: Operation;
  } | null>(null);

  // Resolve the selection against the full scan so the drawer stays in sync
  // after a rescan and survives list filtering.
  const selected = selectedKey
    ? (lastScan?.packages.find((p) => p.key === selectedKey) ?? null)
    : null;

  // When a background operation succeeds, rescan in the background and close
  // the drawer if its package was removed. The list is never cleared while
  // this happens — the app stays usable throughout.
  const settledRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    let needsRefresh = false;
    let removedKey: string | null = null;
    for (const t of tasks) {
      if (t.status === "running" || settledRef.current.has(t.id)) continue;
      settledRef.current.add(t.id);
      if (t.status !== "success") continue;
      needsRefresh = true;
      if (t.kind === "uninstall") removedKey = t.pkgKey;
    }
    if (removedKey && removedKey === selectedKey) {
      setSelectedKey(null);
    }
    if (needsRefresh) {
      refresh();
    }
  }, [tasks, refresh, selectedKey]);

  const handleSelect = (pkg: InstalledPackage) =>
    setSelectedKey((prev) => (prev === pkg.key ? null : pkg.key));

  const handleAction = (pkg: InstalledPackage, kind: Operation) =>
    setConfirmTarget({ pkg, kind });

  return (
    <section className="page">
      <header className="page__head">
        <h1 className="page__title">Apps</h1>
        <p className="page__subtitle">
          See, update, and uninstall everything installed on your system.
        </p>
      </header>

      <PackageFilters
        query={query}
        source={sourceFilter}
        kind={kindFilter}
        viewMode={viewMode}
        updatesCount={updatesCount}
        refreshing={refreshing}
        onQuery={setQuery}
        onSource={setSourceFilter}
        onKind={setKindFilter}
        onViewMode={setViewMode}
        sortMode={sortMode}
        onSortBySize={toggleSortBySize}
        onRescan={refresh}
      />

      {error && <div className="banner banner--error">{error}</div>}
      {lastScan?.availability?.apt_error && !error && (
        <div className="banner banner--warn">
          APT: {lastScan.availability.apt_error}
        </div>
      )}

      <div className="page__body">
        {loading ? (
          <div className="pkg-list" aria-label="Scanning installed packages">
            {Array.from({ length: 8 }, (_, i) => (
              <div key={i} className="pkg-skel" />
            ))}
          </div>
        ) : (
          <PackageList
            packages={packages}
            selectedKey={selected?.key ?? null}
            viewMode={viewMode}
            busyByKey={busyByKey}
            onSelect={handleSelect}
            onAction={handleAction}
          />
        )}

        {selected && (
          <PackageDetail
            pkg={selected}
            busy={busyByKey[selected.key] ?? null}
            onClose={() => setSelectedKey(null)}
            onAction={handleAction}
          />
        )}
      </div>

      {confirmTarget && (
        <ConfirmOperationDialog
          pkg={confirmTarget.pkg}
          kind={confirmTarget.kind}
          onClose={() => setConfirmTarget(null)}
        />
      )}
    </section>
  );
}
