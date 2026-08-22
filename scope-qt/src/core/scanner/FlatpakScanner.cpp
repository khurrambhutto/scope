#include "FlatpakScanner.h"

#include "system/System.h"

#include <QRegularExpression>

namespace scope {

bool FlatpakScanner::isAvailable() const
{
    return System::which(QStringLiteral("flatpak"));
}

namespace flatpak_detail {

// Parses flatpak's human size strings ("1.5 GB", "812,3 kB") with binary
// multipliers, mirroring the Rust parser.
quint64 parseSize(QString raw)
{
    raw = raw.trimmed();
    if (raw.isEmpty())
        return 0;
    raw.replace(QLatin1Char(','), QLatin1Char('.'));

    static const QRegularExpression pattern(QStringLiteral("^([\\d.]+)\\s*(\\S*)$"));
    const auto match = pattern.match(raw);
    if (!match.hasMatch())
        return 0;

    bool ok = false;
    const double value = match.captured(1).toDouble(&ok);
    if (!ok)
        return 0;
    const QString unit = match.captured(2).toUpper();

    double multiplier = 1.0;
    if (unit == QLatin1String("KB") || unit == QLatin1String("K"))
        multiplier = 1024.0;
    else if (unit == QLatin1String("MB") || unit == QLatin1String("M"))
        multiplier = 1024.0 * 1024.0;
    else if (unit == QLatin1String("GB") || unit == QLatin1String("G"))
        multiplier = 1024.0 * 1024.0 * 1024.0;
    else if (unit == QLatin1String("TB") || unit == QLatin1String("T"))
        multiplier = 1024.0 * 1024.0 * 1024.0 * 1024.0;
    else if (!unit.isEmpty() && unit != QLatin1String("B"))
        return 0;

    return static_cast<quint64>(value * multiplier);
}

} // namespace flatpak_detail

using flatpak_detail::parseSize;

ScanOutcome FlatpakScanner::scanScope(InstallScope scope)
{
    ScanOutcome outcome;
    outcome.available = true;
    outcome.source = PackageSource::Flatpak;

    const QString scopeFlag = scope == InstallScope::User ? QStringLiteral("--user")
                                                          : QStringLiteral("--system");
    const ExecOutcome list = System::capture(
        QStringLiteral("flatpak"),
        { QStringLiteral("list"), scopeFlag, QStringLiteral("--app"),
          QStringLiteral("--columns=application,name,version,origin,size,description") },
        System::ScanTimeoutMs);
    if (!list.spawned) {
        outcome.error = QStringLiteral("failed to spawn flatpak");
        return outcome;
    }
    if (!list.exitOk) {
        outcome.error = QStringLiteral("flatpak list failed: %1").arg(list.standardError.trimmed());
        return outcome;
    }

    for (QString record : list.standardOutput.split(QLatin1Char('\n'))) {
        record.remove(QLatin1Char('\r'));
        if (record.trimmed().isEmpty())
            continue;
        const QStringList cols = record.split(QLatin1Char('\t'));
        if (cols.size() < 3)
            continue;

        const QString appId = cols.at(0).trimmed();
        InstalledPackage pkg = InstalledPackage{
            makeKey(PackageSource::Flatpak, appId, scope),
            PackageSource::Flatpak,
            appId,
            scope,
            {},
            std::nullopt,
            std::nullopt,
            {},
            0,
            AppKind::Gui,
            std::nullopt,
            std::nullopt,
            false,
            false,
            std::nullopt,
        };
        pkg.name = appId;
        if (cols.size() > 1 && !cols.at(1).trimmed().isEmpty())
            pkg.displayName = cols.at(1).trimmed();
        if (cols.size() > 2 && !cols.at(2).trimmed().isEmpty())
            pkg.version = cols.at(2).trimmed();

        QString description;
        if (cols.size() > 4)
            pkg.sizeBytes = parseSize(cols.at(4));
        if (cols.size() > 5 && !cols.at(5).trimmed().isEmpty())
            description = cols.at(5).trimmed();
        if (cols.size() > 3 && !cols.at(3).trimmed().isEmpty()) {
            const QString origin = cols.at(3).trimmed();
            description += description.isEmpty()
                ? QStringLiteral("(remote: %1)").arg(origin)
                : QStringLiteral("  (remote: %1)").arg(origin);
        }
        if (!description.isEmpty())
            pkg.description = description;

        outcome.packages.append(std::move(pkg));
    }
    return outcome;
}

ScanOutcome FlatpakScanner::scan() const
{
    ScanOutcome user = scanScope(InstallScope::User);
    ScanOutcome system = scanScope(InstallScope::System);

    const bool userOk = user.error.isEmpty();
    const bool systemOk = system.error.isEmpty();

    if (userOk && systemOk) {
        user.packages += system.packages;
        return user;
    }
    if (userOk)
        return user;
    if (systemOk)
        return system;

    user.error = QStringLiteral("%1; %2").arg(user.error, system.error);
    return user;
}

void FlatpakScanner::checkScopeUpdates(InstallScope scope, QList<InstalledPackage>& packages)
{
    const QString scopeFlag = scope == InstallScope::User ? QStringLiteral("--user")
                                                          : QStringLiteral("--system");

    // Refresh appstream metadata first; result intentionally ignored.
    System::capture(QStringLiteral("flatpak"),
                    { QStringLiteral("update"), QStringLiteral("--appstream") },
                    System::ScanTimeoutMs);

    const ExecOutcome remote =
        System::capture(QStringLiteral("flatpak"),
                        { QStringLiteral("remote-ls"), QStringLiteral("--updates"), scopeFlag,
                          QStringLiteral("--columns=application,version") },
                        System::ScanTimeoutMs);
    if (!remote.spawned || !remote.exitOk)
        return;

    QHash<QString, QString> updates;
    for (QString record : remote.standardOutput.split(QLatin1Char('\n'))) {
        record.remove(QLatin1Char('\r'));
        if (record.trimmed().isEmpty())
            continue;
        const QStringList cols = record.split(QLatin1Char('\t'));
        if (cols.isEmpty() || cols.at(0).trimmed().isEmpty())
            continue;
        updates.insert(cols.at(0).trimmed(),
                       cols.size() > 1 ? cols.at(1).trimmed() : QString());
    }

    for (InstalledPackage& pkg : packages) {
        if (pkg.installScope != scope)
            continue;
        const auto it = updates.constFind(pkg.packageId);
        if (it != updates.cend()) {
            pkg.hasUpdate = true;
            if (!it.value().isEmpty())
                pkg.updateVersion = it.value();
        }
    }
}

void FlatpakScanner::checkUpdates(QList<InstalledPackage>& packages) const
{
    checkScopeUpdates(InstallScope::User, packages);
    checkScopeUpdates(InstallScope::System, packages);
}

} // namespace scope
