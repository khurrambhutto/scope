#include "AptScanner.h"

#include "system/System.h"

#include <QDir>
#include <QFileInfo>
#include <QRegularExpression>

namespace scope {

namespace {

const char kUnitSeparator = '\x1f';

} // namespace

bool AptScanner::isAvailable() const
{
    return System::which(QStringLiteral("dpkg-query")) && System::which(QStringLiteral("apt-mark"));
}

AppKind AptScanner::classify(const QString& rawName)
{
    static const QStringList desktopDirs = {
        QStringLiteral("/usr/share/applications"),
        QStringLiteral("/usr/local/share/applications"),
    };
    const QString lower = rawName.toLower();
    QString underscored = rawName;
    underscored.replace(QLatin1Char('-'), QLatin1Char('_'));

    for (const QString& dir : desktopDirs) {
        if (QFileInfo::exists(dir + QLatin1Char('/') + lower + QStringLiteral(".desktop")))
            return AppKind::Gui;
        if (QFileInfo::exists(dir + QLatin1Char('/') + underscored + QStringLiteral(".desktop")))
            return AppKind::Gui;
    }

    static const QStringList binDirs = {
        QStringLiteral("/usr/bin"), QStringLiteral("/bin"), QStringLiteral("/usr/sbin"),
        QStringLiteral("/sbin"), QStringLiteral("/usr/local/bin"),
    };
    for (const QString& dir : binDirs) {
        if (QFileInfo(dir + QLatin1Char('/') + rawName).isFile())
            return AppKind::Cli;
        if (QFileInfo(dir + QLatin1Char('/') + underscored).isFile())
            return AppKind::Cli;
    }
    return AppKind::Unknown;
}

ScanOutcome AptScanner::scan() const
{
    ScanOutcome outcome;
    outcome.available = true;
    outcome.source = PackageSource::Apt;

    const ExecOutcome manual = System::capture(
        QStringLiteral("apt-mark"), { QStringLiteral("showmanual") }, System::ScanTimeoutMs);
    if (!manual.spawned) {
        outcome.error = QStringLiteral("failed to spawn apt-mark");
        return outcome;
    }
    if (!manual.exitOk) {
        outcome.error = QStringLiteral("apt-mark failed: %1").arg(manual.standardError.trimmed());
        return outcome;
    }

    QSet<QString> names;
    for (QString line : manual.standardOutput.split(QLatin1Char('\n'))) {
        line = line.trimmed();
        if (!line.isEmpty())
            names.insert(line);
    }
    if (names.isEmpty())
        return outcome; // available, but nothing manually installed

    QStringList args;
    const QString format = QStringLiteral("${Package}") + QChar(kUnitSeparator) +
                           QStringLiteral("${Version}") + QChar(kUnitSeparator) +
                           QStringLiteral("${Installed-Size}") + QChar(kUnitSeparator) +
                           QStringLiteral("${binary:Summary}") + QChar(kUnitSeparator) +
                           QLatin1Char('\n');
    args << QStringLiteral("-W") << QStringLiteral("-f") << format;
    for (const QString& name : std::as_const(names))
        args << name;

    const ExecOutcome query =
        System::capture(QStringLiteral("dpkg-query"), args, System::DpkgTimeoutMs);
    if (!query.spawned) {
        outcome.error = QStringLiteral("failed to spawn dpkg-query");
        return outcome;
    }
    // dpkg-query exits non-zero when any requested name is unknown, but still
    // prints every resolvable record to stdout. Only treat a fully empty
    // response as an error.
    if (!query.exitOk && query.standardOutput.trimmed().isEmpty()) {
        outcome.error = QStringLiteral("dpkg-query failed: %1").arg(query.standardError.trimmed());
        return outcome;
    }

    for (const QString& record : query.standardOutput.split(QLatin1Char('\n'))) {
        const QStringList fields = record.split(kUnitSeparator);
        if (fields.size() < 3)
            continue;
        InstalledPackage pkg = InstalledPackage{ makeKey(PackageSource::Apt, fields.at(0)),
                                                 PackageSource::Apt, fields.at(0),
                                                 std::nullopt, {}, std::nullopt,
                                                 std::nullopt, {}, 0, AppKind::Unknown,
                                                 std::nullopt, std::nullopt, false, false,
                                                 std::nullopt };
        pkg.name = fields.at(0);
        pkg.version = fields.at(1);
        bool ok = false;
        const quint64 kib = fields.at(2).toULongLong(&ok);
        pkg.sizeBytes = ok ? kib * 1024 : 0;
        if (fields.size() >= 4 && !fields.at(3).trimmed().isEmpty())
            pkg.description = fields.at(3).trimmed();
        pkg.appKind = classify(pkg.name);
        outcome.packages.append(std::move(pkg));
    }

    checkUpdates(outcome.packages);
    return outcome;
}

void AptScanner::checkUpdates(QList<InstalledPackage>& packages) const
{
    const ExecOutcome upgradable = System::capture(
        QStringLiteral("apt"), { QStringLiteral("list"), QStringLiteral("--upgradable") },
        System::ScanTimeoutMs);
    if (!upgradable.spawned || !upgradable.exitOk)
        return;

    static const QRegularExpression pattern(
        QStringLiteral(R"(^(\S+)/(\S+)\s+(\S+)\s+\S+\s+\[upgradable from: (\S+)\])"));

    QHash<QString, QString> candidates;
    for (QString line : upgradable.standardOutput.split(QLatin1Char('\n'))) {
        line = line.trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1String("Listing")) ||
            line.startsWith(QLatin1String("WARNING")))
            continue;
        const auto match = pattern.match(line);
        if (match.hasMatch())
            candidates.insert(match.captured(1), match.captured(3));
    }

    for (InstalledPackage& pkg : packages) {
        const auto it = candidates.constFind(pkg.packageId);
        if (it != candidates.cend()) {
            pkg.hasUpdate = true;
            pkg.updateVersion = it.value();
        }
    }
}

} // namespace scope
