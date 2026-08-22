#pragma once

#include "package/Package.h"
#include "scanner/Scanner.h"

#include <QObject>

namespace scope {

struct ScanResult {
    bool ok = false;
    QString error;
    CachedScan scan;
};

// Runs all source scanners concurrently on a worker thread and emits the
// merged, enriched package list. UI never blocks on scans.
class ScanService : public QObject
{
    Q_OBJECT

public:
    explicit ScanService(QObject* parent = nullptr);

    void scanAllAsync();

    // Blocking full scan; used by operation revalidation on worker threads.
    [[nodiscard]] ScanResult scanBlocking() const { return runScan(); }

    // Cheap probes only; safe to call from the UI thread.
    [[nodiscard]] static ScanAvailability probeAvailability();

    [[nodiscard]] CachedScan lastScan() const { return m_lastScan; }
    [[nodiscard]] bool isScanning() const { return m_scanning; }

signals:
    void scanStarted();
    void scanFinished(const scope::ScanResult& result);

private:
    [[nodiscard]] ScanResult runScan() const;
    void enrich(QList<InstalledPackage>& packages, class DesktopIndex& index) const;
    void promoteManualApps(const DesktopIndex& index, QList<InstalledPackage>& packages) const;

    CachedScan m_lastScan;
    bool m_scanning = false;
};

} // namespace scope

Q_DECLARE_METATYPE(scope::ScanResult)
