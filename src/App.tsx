import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type PackageSource = "Apt" | "Snap" | "Flatpak" | "AppImage" | "DebFile";
type AppType = "GUI" | "CLI" | "Unknown";
type SourceFilter = "All" | PackageSource;

type ScopePackage = {
  name: string;
  version: string;
  description: string;
  size_bytes: number;
  source: PackageSource;
  app_type: AppType;
  has_update: boolean | null;
  update_version: string | null;
  install_path: string | null;
  aliases: string[];
};

const navItems = [
  "Overview",
  "Clean",
  "Uninstall",
  "Analyze",
  "Optimize",
  "Purge",
  "Installers",
  "Status",
  "History",
  "Settings",
];

const sourceFilters: SourceFilter[] = [
  "All",
  "Apt",
  "Snap",
  "Flatpak",
  "AppImage",
];

const sourceLabels: Record<PackageSource, string> = {
  Apt: "APT",
  Snap: "Snap",
  Flatpak: "Flatpak",
  AppImage: "AppImage",
  DebFile: "Deb",
};

function formatBytes(bytes: number) {
  if (!bytes) return "0 B";

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

function packageMatchesFilter(pkg: ScopePackage, filter: SourceFilter) {
  if (filter === "All") return true;
  if (filter === "Apt") return pkg.source === "Apt" || pkg.source === "DebFile";
  return pkg.source === filter;
}

function App() {
  const [packages, setPackages] = useState<ScopePackage[]>([]);
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("All");
  const [query, setQuery] = useState("");
  const [activeNav, setActiveNav] = useState("Uninstall");
  const [isScanning, setIsScanning] = useState(false);
  const [status, setStatus] = useState("Ready");
  const [error, setError] = useState<string | null>(null);

  async function runScan(includeUpdates = false) {
    setIsScanning(true);
    setError(null);
    setStatus(includeUpdates ? "Checking packages and updates" : "Scanning packages");

    try {
      const command = includeUpdates ? "scan_packages_with_updates" : "scan_packages";
      const result = await invoke<ScopePackage[]>(command);
      setPackages(result);
      setStatus(`${result.length} packages found`);
    } catch (scanError) {
      const message = scanError instanceof Error ? scanError.message : String(scanError);
      setError(message);
      setStatus("Scan failed");
    } finally {
      setIsScanning(false);
    }
  }

  const filteredPackages = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();

    return packages.filter((pkg) => {
      const sourceMatch = packageMatchesFilter(pkg, sourceFilter);
      if (!sourceMatch) return false;
      if (!normalizedQuery) return true;

      return [pkg.name, pkg.description, pkg.version, pkg.install_path, ...pkg.aliases]
        .filter(Boolean)
        .some((value) => value!.toLowerCase().includes(normalizedQuery));
    });
  }, [packages, query, sourceFilter]);

  const totalSize = filteredPackages.reduce((sum, pkg) => sum + pkg.size_bytes, 0);
  const updateCount = packages.filter((pkg) => pkg.has_update).length;
  const guiCount = packages.filter((pkg) => pkg.app_type === "GUI").length;

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">S</span>
          <div>
            <p>Scope</p>
            <span>Linux maintenance</span>
          </div>
        </div>

        <nav aria-label="Primary">
          {navItems.map((item) => (
            <button
              className={item === activeNav ? "nav-item active" : "nav-item"}
              key={item}
              onClick={() => setActiveNav(item)}
              type="button"
            >
              <span>{item}</span>
            </button>
          ))}
        </nav>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <p className="eyebrow">Phase 1</p>
            <h1>{activeNav}</h1>
          </div>
          <div className="actions">
            <button disabled={isScanning} onClick={() => runScan(false)} type="button">
              {isScanning ? "Scanning" : "Scan"}
            </button>
            <button disabled={isScanning} onClick={() => runScan(true)} type="button">
              Updates
            </button>
          </div>
        </header>

        <section className="metrics" aria-label="Package summary">
          <article>
            <span>Packages</span>
            <strong>{filteredPackages.length}</strong>
          </article>
          <article>
            <span>Visible size</span>
            <strong>{formatBytes(totalSize)}</strong>
          </article>
          <article>
            <span>GUI apps</span>
            <strong>{guiCount}</strong>
          </article>
          <article>
            <span>Updates</span>
            <strong>{updateCount}</strong>
          </article>
        </section>

        <section className="table-panel">
          <div className="table-toolbar">
            <div className="source-tabs" role="tablist" aria-label="Package source">
              {sourceFilters.map((filter) => (
                <button
                  className={filter === sourceFilter ? "source-tab active" : "source-tab"}
                  key={filter}
                  onClick={() => setSourceFilter(filter)}
                  type="button"
                >
                  {filter === "Apt" ? "APT" : filter}
                </button>
              ))}
            </div>
            <input
              aria-label="Search packages"
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="Search packages"
              value={query}
            />
          </div>

          <div className="status-line">
            <span>{status}</span>
            {error && <strong>{error}</strong>}
          </div>

          <div className="package-table" role="table" aria-label="Installed packages">
            <div className="table-row table-head" role="row">
              <span>Name</span>
              <span>Source</span>
              <span>Type</span>
              <span>Version</span>
              <span>Size</span>
              <span>Status</span>
            </div>

            {filteredPackages.map((pkg) => (
              <div className="table-row" key={`${pkg.source}-${pkg.name}`} role="row">
                <span className="package-name">
                  <strong>{pkg.name}</strong>
                  <small>{pkg.description || pkg.install_path || "No description"}</small>
                </span>
                <span>
                  <mark className={`source-pill source-${pkg.source.toLowerCase()}`}>
                    {sourceLabels[pkg.source]}
                  </mark>
                </span>
                <span>{pkg.app_type}</span>
                <span>{pkg.version || "unknown"}</span>
                <span>{formatBytes(pkg.size_bytes)}</span>
                <span>{pkg.has_update ? `Update ${pkg.update_version ?? ""}` : "Installed"}</span>
              </div>
            ))}

            {!filteredPackages.length && (
              <div className="empty-state">
                <strong>No packages loaded</strong>
                <span>{isScanning ? "Scan in progress" : "Run a scan to populate this view"}</span>
              </div>
            )}
          </div>
        </section>
      </section>
    </main>
  );
}

export default App;
