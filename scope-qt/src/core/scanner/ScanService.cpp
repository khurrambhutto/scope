#include "ScanService.h"

#include "desktop/DesktopEntry.h"
#include "icons/IconResolver.h"
#include "scanner/AptScanner.h"
#include "scanner/AppImageScanner.h"
#include "scanner/FlatpakScanner.h"
#include "scanner/SnapScanner.h"
#include "system/System.h"

#include <QDateTime>
#include <QDir>
#include <QFileInfo>
#include <QSemaphore>
#include <QThreadPool>

#include <algorithm>
#include <functional>

namespace scope {

namespace {

constexpr char kUnitSeparator = '\x1f';

// Executables claimed by a package manager never become Manual entries.
void collectClaimedBinaries(const QList<InstalledPackage>& packages, QSet<QString>& claimed)
{
    for (const InstalledPackage& pkg : packages) {
        if (pkg.source == PackageSource::AppImage || pkg.source == PackageSource::Snap ||
            pkg.source == PackageSource::Manual) {
            QString id = pkg.packageId.toLower();
            if (pkg.source == PackageSource::AppImage) {
                const QFileInfo info(pkg.packageId);
                const QString canonical = info.canonicalFilePath();
                if (!canonical.isEmpty())
                    id = canonical.toLower();
            }
            claimed.insert(id);
        }
    }
}

bool isExcludedSystemPath(const QString& binary)
{
    if (binary.startsWith(QLatin1String("/snap/")))
        return true;
    if (binary.startsWith(QLatin1String("/usr/")) &&
        !binary.startsWith(QLatin1String("/usr/local/")))
        return true;
    if (binary.startsWith(QLatin1String("/bin/")) || binary.startsWith(QLatin1String("/sbin/")))
        return true;
    return false;
}

} // namespace

ScanService::ScanService(QObject* parent)
    : QObject(parent)
{
}

ScanAvailability ScanService::probeAvailability()
{
    ScanAvailability availability;
    availability.apt = System::which(QStringLiteral("dpkg-query")) &&
                       System::which(QStringLiteral("apt-mark"));
    availability.snap = System::which(QStringLiteral("snap"));
    availability.flatpak = System::which(QStringLiteral("flatpak"));
    availability.appimage = true;
    availability.appimageDirs = AppImageScanner::searchDirectories();
    return availability;
}

void ScanService::scanAllAsync()
{
    if (m_scanning)
        return;
    m_scanning = true;
    emit scanStarted();

    // Copy `this` access into the worker carefully: service outlives the pool
    // task because it is owned by the controller for the app's lifetime.
    QThreadPool::globalInstance()->start([this]() {
        ScanResult result = runScan();
        m_lastScan = result.scan;
        m_scanning = false;
        emit scanFinished(result);
    });
}

ScanResult ScanService::runScan() const
{
    ScanResult result;
    result.ok = true;

    DesktopIndex index(DesktopIndex::collect());

    const AptScanner apt;
    const SnapScanner snap;
    const FlatpakScanner flatpak;
    const AppImageScanner appimage;

    struct Job {
        PackageSource source;
        std::function<ScanOutcome()> run;
    };
    const QVector<Job> jobs = {
        { PackageSource::Apt, [&apt] {
             ScanOutcome o;
             o.source = PackageSource::Apt;
             o.available = apt.isAvailable();
             return o.available ? apt.scan() : o;
         } },
        { PackageSource::Snap, [&snap] {
             ScanOutcome o;
             o.source = PackageSource::Snap;
             o.available = snap.isAvailable();
             return o.available ? snap.scan() : o;
         } },
        { PackageSource::Flatpak, [&flatpak] {
             ScanOutcome o;
             o.source = PackageSource::Flatpak;
             o.available = flatpak.isAvailable();
             return o.available ? flatpak.scan() : o;
         } },
        { PackageSource::AppImage, [&appimage] {
             ScanOutcome o;
             o.source = PackageSource::AppImage;
             o.available = appimage.isAvailable();
             return o.available ? appimage.scan() : o;
         } },
    };

    QVector<ScanOutcome> outcomes(jobs.size());
    QAtomicInt remaining{ static_cast<int>(jobs.size()) };
    QSemaphore done(0);

    for (int i = 0; i < jobs.size(); ++i) {
        QThreadPool::globalInstance()->start([&outcomes, &jobs, &remaining, &done, i]() {
            outcomes[i] = jobs[i].run();
            if (remaining.fetchAndSubRelaxed(1) == 1)
                done.release();
        });
    }
    done.acquire();

    QList<InstalledPackage> packages;
    for (const ScanOutcome& outcome : outcomes) {
        switch (outcome.source) {
        case PackageSource::Apt:
            result.scan.availability.apt = outcome.available || !outcome.error.isEmpty();
            if (!outcome.error.isEmpty()) {
                result.scan.availability.apt = true;
                result.scan.availability.aptError = outcome.error;
            } else {
                result.scan.availability.apt = outcome.available;
            }
            break;
        case PackageSource::Snap:
            result.scan.availability.snap = outcome.available;
            if (!outcome.error.isEmpty())
                result.scan.availability.snapError = outcome.error;
            break;
        case PackageSource::Flatpak:
            result.scan.availability.flatpak = outcome.available;
            if (!outcome.error.isEmpty())
                result.scan.availability.flatpakError = outcome.error;
            break;
        case PackageSource::AppImage:
            result.scan.availability.appimage = outcome.available;
            break;
        default:
            break;
        }
        packages += outcome.packages;
    }

    enrich(packages, index);
    promoteManualApps(index, packages);
    result.scan.availability.manual = true;

    std::sort(packages.begin(), packages.end(),
              [](const InstalledPackage& a, const InstalledPackage& b) {
                  const int rankA = appKindRank(a.appKind);
                  const int rankB = appKindRank(b.appKind);
                  if (rankA != rankB)
                      return rankA < rankB;
                  return a.effectiveName().compare(b.effectiveName(), Qt::CaseInsensitive) < 0;
              });

    result.scan.packages = std::move(packages);
    result.scan.scannedAtMs = QDateTime::currentMSecsSinceEpoch();
    return result;
}

void ScanService::enrich(QList<InstalledPackage>& packages, DesktopIndex& index) const
{
    for (InstalledPackage& pkg : packages) {
        if (pkg.source == PackageSource::Manual)
            continue; // created from desktop entries already

        std::optional<DesktopApp> entry;
        switch (pkg.source) {
        case PackageSource::Flatpak:
            entry = index.lookupFlatpak(pkg.packageId);
            break;
        case PackageSource::Snap:
            entry = index.lookupSnap(pkg.packageId);
            break;
        default:
            entry = index.lookupGeneric(pkg.packageId, pkg.displayName.value_or(QString()));
            break;
        }

        if (entry) {
            pkg.displayName = entry->name;
            if (!pkg.icon) {
                const auto resolved = IconResolver::instance().resolve(entry->icon.value_or(QString()));
                if (resolved)
                    pkg.icon = IconResolver::instance().iconUrl(*resolved);
            }
            if ((!pkg.description || pkg.description->isEmpty()) && entry->comment &&
                !entry->comment->isEmpty()) {
                pkg.description = entry->comment;
            }
            if ((!pkg.categories || pkg.categories->isEmpty()) && !entry->categories.isEmpty())
                pkg.categories = entry->categories.join(QLatin1Char(','));
            pkg.terminal = entry->terminal;
            if (!entry->terminal)
                pkg.appKind = AppKind::Gui;
        }

        // AppImage icons come from the file itself when possible.
        if (!pkg.icon && pkg.source == PackageSource::AppImage) {
            const auto resolved = IconResolver::instance().resolve(pkg.packageId);
            if (resolved)
                pkg.icon = IconResolver::instance().iconUrl(*resolved);
        }
    }
}

void ScanService::promoteManualApps(const DesktopIndex& index,
                                    QList<InstalledPackage>& packages) const
{
    QSet<QString> claimed;
    collectClaimedBinaries(packages, claimed);

    for (const DesktopApp& entry : index.unmatched()) {
        if (entry.terminal)
            continue;

        const auto token = execBinaryToken(entry.exec);
        if (!token)
            continue;
        const auto binary = resolveExecBinary(*token);
        if (!binary)
            continue;

        const QString resolved = *binary;
        const QFileInfo info(resolved);
        const QString canonical = info.canonicalFilePath();

        if (claimed.contains(resolved.toLower()) ||
            (!canonical.isEmpty() && claimed.contains(canonical.toLower())))
            continue;
        if (isExcludedSystemPath(resolved))
            continue;
        if (!info.isExecutable())
            continue;

        InstalledPackage pkg;
        pkg.key = QStringLiteral("manual:") + entry.id;
        pkg.source = PackageSource::Manual;
        pkg.packageId = resolved + QChar(kUnitSeparator) + entry.path;
        pkg.name = entry.name;
        pkg.version = QStringLiteral("unknown");
        pkg.sizeBytes = static_cast<quint64>(info.size());
        pkg.appKind = AppKind::Gui;
        pkg.terminal = false;

        const auto iconResolved =
            IconResolver::instance().resolve(entry.icon.value_or(QString()));
        if (iconResolved)
            pkg.icon = IconResolver::instance().iconUrl(*iconResolved);
        if (entry.comment && !entry.comment->isEmpty())
            pkg.description = entry.comment;
        if (!entry.categories.isEmpty())
            pkg.categories = entry.categories.join(QLatin1Char(','));

        packages.append(std::move(pkg));
    }
}

} // namespace scope
