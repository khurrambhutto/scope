#include "AppImageScanner.h"

#include "system/System.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QRegularExpression>

namespace scope {

namespace {

QString extractName(QString stem)
{
    stem.chop(QStringLiteral(".appimage").size());
    static const QRegularExpression tail(
        QStringLiteral("[-_]?(v?\\d[\\d.]*|x86_64|amd64|aarch64|arm64|linux).*$"),
        QRegularExpression::CaseInsensitiveOption);
    QString name = stem;
    name.remove(tail);
    while (!name.isEmpty() && QStringLiteral("-_ .").contains(name.back()))
        name.chop(1);
    if (name.isEmpty())
        return stem;
    return name;
}

QString extractVersion(const QString& stem)
{
    static const QRegularExpression version(
        QStringLiteral("[_-]v?(\\d+(?:\\.\\d+){1,3})"));
    const auto match = version.match(stem);
    if (match.hasMatch())
        return match.captured(1);
    return QStringLiteral("unknown");
}

} // namespace

QStringList AppImageScanner::searchDirectories()
{
    QStringList dirs = {
        QStringLiteral("/opt"),
        QStringLiteral("/usr/local/bin"),
        QDir::homePath() + QStringLiteral("/Applications"),
        QDir::homePath() + QStringLiteral("/apps"),
        QDir::homePath() + QStringLiteral("/AppImages"),
        QDir::homePath() + QStringLiteral("/Downloads"),
        QDir::homePath() + QStringLiteral("/.local/bin"),
    };
    QStringList unique;
    for (const QString& dir : dirs) {
        if (!unique.contains(dir))
            unique << dir;
    }
    return unique;
}

bool AppImageScanner::looksLikeAppImage(const QString& path)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return false;
    const QByteArray magic = file.read(11);
    if (magic.size() < 11)
        return false;
    const uchar* bytes = reinterpret_cast<const uchar*>(magic.constData());
    return bytes[0] == 0x7F && bytes[1] == 'E' && bytes[2] == 'L' && bytes[3] == 'F' &&
           bytes[8] == 'A' && bytes[9] == 'I' && (bytes[10] == 0x01 || bytes[10] == 0x02);
}

InstalledPackage AppImageScanner::buildPackage(const QString& path)
{
    const QFileInfo info(path);
    const QString stem = info.fileName();

    InstalledPackage pkg{ makeKey(PackageSource::AppImage, info.absoluteFilePath()),
                          PackageSource::AppImage,
                          info.absoluteFilePath(),
                          std::nullopt,
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
                          std::nullopt };
    pkg.name = extractName(stem);
    pkg.displayName = pkg.name; // preset before desktop enrichment
    pkg.version = extractVersion(stem);
    pkg.sizeBytes = static_cast<quint64>(info.size());
    return pkg;
}

ScanOutcome AppImageScanner::scan() const
{
    ScanOutcome outcome;
    outcome.source = PackageSource::AppImage;

    for (const QString& dir : searchDirectories()) {
        QDir root(dir);
        if (!root.exists())
            continue;

        QDirIterator it(dir, QDir::Files | QDir::NoDotAndDotDot, QDirIterator::Subdirectories);
        int depthGuard = 0; // QDirIterator has no max depth; enforce manually below
        Q_UNUSED(depthGuard);

        while (it.hasNext()) {
            const QString path = it.next();
            const QFileInfo info(path);

            // Skip hidden files/dirs anywhere in the path below the root.
            const QString relative = root.relativeFilePath(path);
            bool hidden = false;
            for (const QString& part : relative.split(QLatin1Char('/'))) {
                if (part.startsWith(QLatin1Char('.'))) {
                    hidden = true;
                    break;
                }
            }
            if (hidden)
                continue;

            // Enforce max depth of 3.
            if (relative.count(QLatin1Char('/')) >= 3)
                continue;

            if (!info.fileName().endsWith(QLatin1String(".appimage"), Qt::CaseInsensitive))
                continue;
            if (!looksLikeAppImage(path))
                continue;

            outcome.packages.append(buildPackage(path));
        }
    }
    return outcome;
}

} // namespace scope
