#include "SnapScanner.h"

#include "system/System.h"

#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>

namespace scope {

bool SnapScanner::isAvailable() const
{
    return System::which(QStringLiteral("snap")) && QDir(QStringLiteral("/var/lib/snapd")).exists();
}

bool SnapScanner::isRuntime(const QString& name)
{
    return name == QLatin1String("snapd") || name == QLatin1String("bare") ||
           name.startsWith(QLatin1String("core")) || name.startsWith(QLatin1String("gtk-")) ||
           name.startsWith(QLatin1String("gnome-")) || name.endsWith(QLatin1String("-gtk3"));
}

quint64 SnapScanner::installedSize(const QString& snapName)
{
    const QString path =
        QStringLiteral("/snap/") + snapName + QStringLiteral("/current");
    if (!QFileInfo::exists(path))
        return 0;
    const ExecOutcome du = System::capture(QStringLiteral("du"),
                                           { QStringLiteral("-sbL"), path },
                                           System::DuTimeoutMs);
    if (!du.spawned || du.standardOutput.trimmed().isEmpty())
        return 0;
    bool ok = false;
    const quint64 bytes = du.standardOutput.split(QLatin1Char(' ')).first().toULongLong(&ok);
    return ok ? bytes : 0;
}

ScanOutcome SnapScanner::scan() const
{
    ScanOutcome outcome;
    outcome.available = true;
    outcome.source = PackageSource::Snap;

    const ExecOutcome list =
        System::capture(QStringLiteral("snap"), { QStringLiteral("list") }, System::ScanTimeoutMs);
    if (!list.spawned) {
        outcome.error = QStringLiteral("failed to spawn snap");
        return outcome;
    }
    if (!list.exitOk) {
        outcome.error = QStringLiteral("snap list failed: %1").arg(list.standardError.trimmed());
        return outcome;
    }

    const QStringList lines = list.standardOutput.split(QLatin1Char('\n'));
    for (int i = 1; i < lines.size(); ++i) { // skip header
        const QStringList cols = lines.at(i).split(QRegularExpression(QStringLiteral("\\s+")),
                                                   Qt::SkipEmptyParts);
        if (cols.size() < 4)
            continue;
        const QString name = cols.at(0);
        if (isRuntime(name))
            continue;

        InstalledPackage pkg{ makeKey(PackageSource::Snap, name),
                              PackageSource::Snap,
                              name,
                              std::nullopt,
                              {},
                              std::nullopt,
                              std::nullopt,
                              {},
                              0,
                              AppKind::Unknown,
                              std::nullopt,
                              std::nullopt,
                              false,
                              false,
                              std::nullopt };
        pkg.name = name;
        pkg.version = cols.at(1);
        pkg.appKind = QFileInfo(QStringLiteral("/snap/bin/") + name).isFile() ? AppKind::Cli
                                                                             : AppKind::Unknown;
        pkg.sizeBytes = installedSize(name);
        outcome.packages.append(std::move(pkg));
    }

    checkUpdates(outcome.packages);
    return outcome;
}

void SnapScanner::checkUpdates(QList<InstalledPackage>& packages) const
{
    const ExecOutcome refresh = System::capture(
        QStringLiteral("snap"), { QStringLiteral("refresh"), QStringLiteral("--list") },
        System::ScanTimeoutMs);
    if (!refresh.spawned || !refresh.exitOk)
        return;

    QSet<QString> updatable;
    const QStringList lines = refresh.standardOutput.split(QLatin1Char('\n'));
    for (int i = 1; i < lines.size(); ++i) { // skip header
        const QStringList cols = lines.at(i).split(QRegularExpression(QStringLiteral("\\s+")),
                                                   Qt::SkipEmptyParts);
        if (cols.size() >= 4)
            updatable.insert(cols.at(0));
    }

    for (InstalledPackage& pkg : packages) {
        if (updatable.contains(pkg.packageId))
            pkg.hasUpdate = true; // target version unknown from this output
    }
}

} // namespace scope
