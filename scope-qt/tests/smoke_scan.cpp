#include "operations/OperationTypes.h"
#include "operations/PlanStore.h"
#include "operations/UninstallOp.h"
#include "operations/UpdateOp.h"
#include "package/Package.h"
#include "safety/Safety.h"
#include "scanner/ScanService.h"

#include "models/PackageListModel.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QTimer>

#include <cstdio>

using namespace scope;

static void printPlan(const OperationPlan& plan)
{
    std::printf("  plan op=%s source=%s pkg=%s protected=%d auth=%s\n",
                qPrintable(operationId(plan.operation)), qPrintable(sourceId(plan.source)),
                qPrintable(plan.packageId), int(plan.isProtected),
                qPrintable(plan.authMethod == AuthMethod::Pkexec ? "pkexec" : "none"));
    for (const PlanStep& step : plan.steps)
        std::printf("    - %s | $ %s\n", qPrintable(step.description),
                    qPrintable(step.commandSummary));
}

int main(int argc, char* argv[])
{
    QCoreApplication app(argc, argv);

    ScanService service;
    QObject::connect(&service, &ScanService::scanFinished,
                     [&](const ScanResult& result) {
                         if (!result.ok) {
                             std::printf("SCAN FAILED: %s\n", qPrintable(result.error));
                             QCoreApplication::exit(1);
                             return;
                         }

                         const CachedScan& scan = result.scan;
                         std::printf("scan ok: %lld packages in %lldms\n",
                                     static_cast<long long>(scan.packages.size()),
                                     static_cast<long long>(
                                         QDateTime::currentMSecsSinceEpoch() -
                                         scan.scannedAtMs));

                         const auto availability = scan.availability;
                         std::printf("availability: apt=%d snap=%d flatpak=%d\n",
                                     int(availability.apt), int(availability.snap),
                                     int(availability.flatpak));

                         int bySource[5] = { 0, 0, 0, 0, 0 };
                         int updates = 0;
                         int guis = 0;
                         int icons = 0;
                         for (const InstalledPackage& pkg : scan.packages) {
                             ++bySource[int(pkg.source)];
                             if (pkg.hasUpdate)
                                 ++updates;
                             if (pkg.appKind == AppKind::Gui)
                                 ++guis;
                             if (pkg.icon)
                                 ++icons;
                         }
                         std::printf("apt=%d snap=%d flatpak=%d appimage=%d manual=%d\n",
                                     bySource[0], bySource[1], bySource[2], bySource[3],
                                     bySource[4]);
                         std::printf("updates=%d gui=%d with-icons=%d\n", updates, guis, icons);

                         // Sample rows from each source.
                         for (int shown = 0, i = 0;
                              i < scan.packages.size() && shown < 8; ++i) {
                             const InstalledPackage& pkg = scan.packages.at(i);
                             std::printf("  [%s] %s v%s (%s) upd=%d\n",
                                         qPrintable(sourceId(pkg.source)),
                                         qPrintable(pkg.effectiveName()),
                                         qPrintable(pkg.version),
                                         qPrintable(formatSize(pkg.sizeBytes)),
                                         int(pkg.hasUpdate));
                             ++shown;
                         }

                         // Preview checks -------------------------------------------------
                         // 1) Protected package must produce a blocked plan.
                         InstalledPackage fake;
                         fake.source = PackageSource::Apt;
                         fake.packageId = QStringLiteral("systemd");
                         fake.name = QStringLiteral("systemd");
                         const OperationPlan blocked = uninstall::preview(fake);
                         std::printf("\nprotected preview: blocked=%d reason=%s\n",
                                     int(blocked.isProtected),
                                     qPrintable(blocked.protectionReason.value_or(QString())));

                         // 2) Real package preview (first non-protected entry).
                         for (const InstalledPackage& pkg : scan.packages) {
                             if (pkg.source == PackageSource::Flatpak ||
                                 pkg.source == PackageSource::Snap || pkg.hasUpdate) {
                                 continue;
                             }
                             const Protection p =
                                 checkPackage(pkg.source, pkg.packageId);
                             if (p.isProtected)
                                 continue;
                             std::printf("\nuninstall preview for %s:\n",
                                         qPrintable(pkg.effectiveName()));
                             printPlan(uninstall::preview(pkg));
                             break;
                         }

                         // 3) Update preview for first updatable package.
                         for (const InstalledPackage& pkg : scan.packages) {
                             if (!pkg.hasUpdate)
                                 continue;
                             std::printf("\nupdate preview for %s:\n",
                                         qPrintable(pkg.effectiveName()));
                             printPlan(update::preview(pkg));
                             break;
                         }

                         // 4) Stale-plan rejection: unknown id must fail.
                         PlanStore store;
                         const auto missing = store.take(QStringLiteral("plan-0-0"));
                         std::printf("\nstale-plan take(unknown)=%d\n", int(missing.has_value()));

                         QCoreApplication::exit(0);
                     });

    QTimer::singleShot(120'000, []() {
        std::printf("TIMEOUT waiting for scan\n");
        QCoreApplication::exit(2);
    });

    service.scanAllAsync();
    return app.exec();
}
