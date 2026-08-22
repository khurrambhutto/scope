#include "IconResolver.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QProcess>
#include <QSettings>

namespace scope {

namespace {

const char* const kIconExtensions[] = { "svg", "png", "xpm" };
const char* const kCandidateExtensions[] = {
    "png", "svg", "svgz", "xpm", "jpg", "jpeg", "webp", "gif", "bmp", "ico",
};

QStringList iconBaseDirs()
{
    QStringList dirs;
    const QString dataHome = qEnvironmentVariable("XDG_DATA_HOME");
    dirs << (dataHome.isEmpty() ? QDir::homePath() + QStringLiteral("/.local/share/icons")
                                : dataHome + QStringLiteral("/icons"));
    dirs << QDir::homePath() + QStringLiteral("/.icons");

    QString dataDirs = qEnvironmentVariable("XDG_DATA_DIRS");
    if (dataDirs.isEmpty())
        dataDirs = QStringLiteral("/usr/local/share:/usr/share");
    for (const QString& part : dataDirs.split(QLatin1Char(':'), Qt::SkipEmptyParts))
        dirs << part + QStringLiteral("/icons");
    return dirs;
}

QString percentEncodePath(const QString& path)
{
    static const QString keep =
        QStringLiteral("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~/");
    const QByteArray utf8 = path.toUtf8();
    QString encoded;
    encoded.reserve(utf8.size() * 2);
    for (char rawByte : utf8) {
        const uchar byte = static_cast<uchar>(rawByte);
        if (byte < 128 && keep.contains(QChar(byte))) {
            encoded += QChar(byte);
        } else {
            encoded += QChar('%');
            encoded += QStringLiteral("%1").arg(byte, 2, 16, QLatin1Char('0')).toUpper();
        }
    }
    return encoded;
}

QString percentDecodePath(const QString& encoded)
{
    QByteArray bytes;
    bytes.reserve(encoded.size());
    for (int i = 0; i < encoded.size(); ++i) {
        const QChar ch = encoded.at(i);
        if (ch == QLatin1Char('%') && i + 2 < encoded.size()) {
            bool ok = false;
            const int value = QStringView(encoded).mid(i + 1, 2).toInt(&ok, 16);
            if (ok) {
                bytes.append(static_cast<char>(value));
                i += 2;
                continue;
            }
        }
        bytes.append(ch.toLatin1());
    }
    return QString::fromUtf8(bytes);
}

} // namespace

// Public decode helper used by the image provider.
namespace iconurl {
QString decode(const QString& encoded)
{
    return percentDecodePath(encoded);
}
QString encode(const QString& path)
{
    return percentEncodePath(path);
}
} // namespace iconurl

IconResolver& IconResolver::instance()
{
    static IconResolver resolver;
    return resolver;
}

bool IconResolver::isIconCandidate(const QString& path)
{
    const QFileInfo info(path);
    const QString suffix = info.suffix().toLower();
    for (const char* ext : kCandidateExtensions) {
        if (suffix == QLatin1String(ext))
            return true;
    }
    if (suffix.isEmpty()) {
        const QString parentName = info.dir().dirName();
        return parentName == QLatin1String(".icons") || parentName == QLatin1String("pixmaps");
    }
    return false;
}

QString IconResolver::activeTheme() const
{
    // 1) gsettings (GNOME)
    QProcess gsettings;
    gsettings.start(QStringLiteral("gsettings"),
                    { QStringLiteral("get"), QStringLiteral("org.gnome.desktop.interface"),
                      QStringLiteral("icon-theme") });
    if (gsettings.waitForFinished(1500)) {
        const QString raw = QString::fromLocal8Bit(gsettings.readAllStandardOutput()).trimmed();
        QString theme = raw;
        if (theme.startsWith(QLatin1Char('\'')) && theme.endsWith(QLatin1Char('\'')) &&
            theme.size() >= 2) {
            theme = theme.mid(1, theme.size() - 2);
        }
        if (!theme.isEmpty()) {
            for (const QString& base : iconBaseDirs()) {
                if (QDir(base + QLatin1Char('/') + theme).exists())
                    return theme;
            }
        }
    }

    // 2) gtk-3.0 settings.ini
    const QString iniPath =
        QDir::homePath() + QStringLiteral("/.config/gtk-3.0/settings.ini");
    if (QFile::exists(iniPath)) {
        QSettings ini(iniPath, QSettings::IniFormat);
        const QString theme = ini.value(QStringLiteral("gtk-icon-theme-name")).toString();
        if (!theme.isEmpty())
            return theme;
    }
    return {};
}

std::optional<QString> IconResolver::lookupInTheme(const QString& theme, const QString& name,
                                                   QHash<QString, bool>& parentCache) const
{
    if (theme.isEmpty())
        return std::nullopt;

    static const QStringList sizes = {
        QStringLiteral("512x512"), QStringLiteral("256x256"), QStringLiteral("128x128"),
        QStringLiteral("64x64"),   QStringLiteral("48x48"),   QStringLiteral("32x32"),
        QStringLiteral("24x24"),   QStringLiteral("22x22"),   QStringLiteral("16x16"),
        QStringLiteral("scalable"),
    };
    static const QStringList contexts = { QStringLiteral("apps"), QString() };

    for (const QString& base : iconBaseDirs()) {
        for (const QString& size : sizes) {
            for (const QString& context : contexts) {
                QString dirPath = base + QLatin1Char('/') + theme + QLatin1Char('/') + size;
                if (!context.isEmpty())
                    dirPath += QLatin1Char('/') + context;
                for (const char* ext : kIconExtensions) {
                    const QString candidate = dirPath + QLatin1Char('/') + name +
                                              QLatin1Char('.') + QString::fromLatin1(ext);
                    if (QFileInfo::exists(candidate))
                        return candidate;
                }
            }
        }
    }

    // Recurse into parent themes declared in index.theme.
    parentCache.insert(theme.toLower(), true);
    const QString indexTheme = [&]() -> QString {
        for (const QString& base : iconBaseDirs()) {
            const QString candidate =
                base + QLatin1Char('/') + theme + QStringLiteral("/index.theme");
            if (QFile::exists(candidate))
                return candidate;
        }
        return {};
    }();
    if (indexTheme.isEmpty())
        return std::nullopt;

    QSettings ini(indexTheme, QSettings::IniFormat);
    ini.beginGroup(QStringLiteral("Icon Theme"));
    const QStringList parents =
        ini.value(QStringLiteral("Inherits")).toString().split(QLatin1Char(','), Qt::SkipEmptyParts);
    ini.endGroup();

    for (QString parent : parents) {
        parent = parent.trimmed();
        if (parent.isEmpty() || parentCache.contains(parent.toLower()))
            continue;
        if (auto hit = lookupInTheme(parent, name, parentCache))
            return hit;
    }
    return std::nullopt;
}

std::optional<QString> IconResolver::lookupInPixmaps(const QString& name) const
{
    const QStringList pixmapDirs = {
        QDir::homePath() + QStringLiteral("/AppImages/.icons"),
        QDir::homePath() + QStringLiteral("/.local/share/pixmaps"),
        QDir::homePath() + QStringLiteral("/.icons"),
    };
    for (const QString& dir : pixmapDirs) {
        for (const char* ext : kIconExtensions) {
            const QString candidate =
                dir + QLatin1Char('/') + name + QLatin1Char('.') + QString::fromLatin1(ext);
            if (QFileInfo::exists(candidate))
                return candidate;
        }
        const QString extensionless = dir + QLatin1Char('/') + name;
        if (isIconCandidate(extensionless) && QFileInfo(extensionless).isFile())
            return extensionless;
    }

    for (const char* ext : kIconExtensions) {
        const QString candidate = QStringLiteral("/usr/share/pixmaps/") + name +
                                  QLatin1Char('.') + QString::fromLatin1(ext);
        if (QFileInfo::exists(candidate))
            return candidate;
    }
    const QString extensionless = QStringLiteral("/usr/share/pixmaps/") + name;
    if (isIconCandidate(extensionless) && QFileInfo(extensionless).isFile())
        return extensionless;

    return std::nullopt;
}

std::optional<QString> IconResolver::resolveUncached(const QString& iconName)
{
    if (iconName.trimmed().isEmpty())
        return std::nullopt;

    // 1) Absolute path in Icon=
    if (iconName.startsWith(QLatin1Char('/'))) {
        if (isIconCandidate(iconName) && QFileInfo(iconName).isFile())
            return iconName;
        QFileInfo info(iconName);
        const QString suffix = info.suffix();
        for (const char* ext : kCandidateExtensions) {
            const QString swapped =
                iconName.chopped(suffix.size()) + QString::fromLatin1(ext);
            if (suffix.isEmpty())
                break; // extensionless absolute paths: no retry variants
            if (QFileInfo(swapped).isFile() && isIconCandidate(swapped))
                return swapped;
        }
        return std::nullopt;
    }

    // 2) Theme lookup: active theme then hicolor.
    QHash<QString, bool> parentCache;
    const QString active = activeTheme();
    if (!active.isEmpty()) {
        if (auto hit = lookupInTheme(active, iconName, parentCache))
            return hit;
    }
    if (auto hit = lookupInTheme(QStringLiteral("hicolor"), iconName, parentCache))
        return hit;

    // 3) Pixmaps fallback.
    return lookupInPixmaps(iconName);
}

std::optional<QString> IconResolver::resolve(const QString& iconName)
{
    const auto it = m_cache.constFind(iconName);
    if (it != m_cache.cend())
        return it.value();
    auto resolved = resolveUncached(iconName);
    m_cache.insert(iconName, resolved);
    return resolved;
}

QString IconResolver::iconUrl(const QString& resolvedPath)
{
    if (resolvedPath.isEmpty() || !isIconCandidate(resolvedPath))
        return {};
    const QFileInfo info(resolvedPath);
    if (!info.isFile())
        return {};
    const QString canonical = info.canonicalFilePath();
    if (canonical.isEmpty())
        return {};
    m_servedPaths.insert(canonical);
    return QStringLiteral("image://scopeicon/") + iconurl::encode(canonical);
}

bool IconResolver::isRegisteredPath(const QString& path)
{
    const QString canonical = QFileInfo(path).canonicalFilePath();
    if (canonical.isEmpty())
        return false;
    return instance().m_servedPaths.contains(canonical);
}

} // namespace scope
