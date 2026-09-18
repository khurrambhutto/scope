import { useCallback, useMemo, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { Logo } from "../../shared/components/Logo";
import { PackageList } from "./PackageList";
import { PackageFilters } from "./PackageFilters";
import { usePackages } from "./usePackages";

export function PackageScreen() {
  const {
    loading,
    refreshing,
    error,
    packages,
    lastScan,
    query,
    sourceFilter,
    kindFilter,
    viewMode,
    refresh,
    setQuery,
    setSourceFilter,
    setKindFilter,
    setViewMode,
  } = usePackages();
  const [selected, setSelected] = useState<InstalledPackage | null>(null);

  const handleSelect = useCallback(
    (pkg: InstalledPackage) => setSelected((prev) => (prev?.key === pkg.key ? null : pkg)),
    []
  );

  // Keep the selected detail row in sync after a rescan.
  const selectedRow = useMemo(
    () => (selected ? packages.find((p) => p.key === selected.key) ?? selected : null),
    [packages, selected]
  );

  const handleUninstalled = useCallback(
    (pkg: InstalledPackage) => {
      setSelected((prev) => (prev?.key === pkg.key ? null : prev));
      refresh();
    },
    [refresh]
  );

  // A source that timed out or errored should say so, instead of looking like
  // "no packages".
  const sourceWarnings = useMemo(() => {
    const availability = lastScan?.availability;
    return [
      { label: "APT", message: availability?.apt_error },
      { label: "Snap", message: availability?.snap_error },
      { label: "Flatpak", message: availability?.flatpak_error },
    ].filter((warning): warning is { label: string; message: string } =>
      Boolean(warning.message)
    );
  }, [lastScan]);

  return (
    <section className="screen">
      <header className="topbar">
        <div className="topbar__brand">
          <Logo size={28} className="topbar__logo" ariaLabel="Scope" />
          <h1>Scope</h1>
        </div>
      </header>

      <PackageFilters
        query={query}
        source={sourceFilter}
        kind={kindFilter}
        viewMode={viewMode}
        refreshing={refreshing}
        onQuery={setQuery}
        onSource={setSourceFilter}
        onKind={setKindFilter}
        onViewMode={setViewMode}
        onRescan={refresh}
      />

      {error && <div className="banner banner--error">{error}</div>}
      {!error &&
        sourceWarnings.map((warning) => (
          <div className="banner banner--warn" key={warning.label}>
            {warning.label}: {warning.message}
          </div>
        ))}

      <div className="screen__body">
        {loading ? (
          <div className="pkg-list pkg-list--loading">
            Scanning installed apps across APT, Snap, Flatpak, and AppImage…
          </div>
        ) : (
          <PackageList
            packages={packages}
            selectedKey={selectedRow?.key ?? null}
            selectedPkg={selectedRow}
            viewMode={viewMode}
            onSelect={handleSelect}
            onUninstalled={handleUninstalled}
          />
        )}
      </div>

      <footer className="footer" />
    </section>
  );
}
