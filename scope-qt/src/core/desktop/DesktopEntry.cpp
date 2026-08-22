#include "DesktopEntry.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QTextStream>

#include <algorithm>

namespace scope {

namespace {

const QStringList& blacklistedDirSubstrings()
{
    static const QStringList dirs = {
        QStringLiteral("/usr/share/locale"),
        QStringLiteral("/usr/share/app-install"),
        QStringLiteral("/usr/share/kservices5"),
        QStringLiteral("/usr/share/kf5"),
        QStringLiteral("/usr/share/kservicetypes5"),
        QStringLiteral("/usr/share/applications/screensavers"),
        QStringLiteral("/usr/share/kde4"),
        QStringLiteral("/usr/share/mimelnk"),
    };
    return dirs;
}

QStringList applicationDirs()
{
    QStringList dirs;

    const QString dataHome = qEnvironmentVariable("XDG_DATA_HOME");
    if (!dataHome.isEmpty())
        dirs << dataHome + QStringLiteral("/applications");
    else
        dirs << QDir::homePath() + QStringLiteral("/.local/share/applications");

    QString dataDirs = qEnvironmentVariable("XDG_DATA_DIRS");
    if (dataDirs.isEmpty())
        dataDirs = QStringLiteral("/usr/local/share:/usr/share");
    const QStringList parts = dataDirs.split(QLatin1Char(':'), Qt::SkipEmptyParts);
    for (const QString& part : parts)
        dirs << part + QStringLiteral("/applications");

    dirs << QStringLiteral("/var/lib/snapd/desktop/applications");
    return dirs;
}

// Desktop id: relative path components joined with '-', minus .desktop suffix.
QString desktopIdFor(const QString& root, const QString& filePath)
{
    QString rel = QDir(root).relativeFilePath(filePath);
    if (rel.startsWith(QLatin1String("./")))
        rel.remove(0, 2);
    if (rel.endsWith(QLatin1String(".desktop"), Qt::CaseInsensitive))
        rel.chop(QStringLiteral(".desktop").size());
    rel.replace(QLatin1Char('/'), QLatin1Char('-'));
    return rel;
}

bool isFieldCode(const QString& token)
{
    static const QStringList codes = {
        QStringLiteral("%u"), QStringLiteral("%f"), QStringLiteral("%F"),
        QStringLiteral("%U"), QStringLiteral("%i"), QStringLiteral("%c"),
        QStringLiteral("%k"),
    };
    return codes.contains(token);
}

} // namespace

std::optional<DesktopApp> parseDesktopFile(const QString& path, const QString& desktopId)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text))
        return std::nullopt;

    bool inMainSection = false;
    bool sawMainSection = false;
    QHash<QString, QString> values; // bare keys only
    QList<QPair<QString, QString>> localeEntries; // "Name[en]", value

    QTextStream stream(&file);
    while (!stream.atEnd()) {
        QString line = stream.readLine().trimmed();
        if (line.isEmpty() || line.startsWith(QLatin1Char('#')))
            continue;
        if (line.startsWith(QLatin1Char('['))) {
            inMainSection = line == QLatin1String("[Desktop Entry]");
            if (inMainSection)
                sawMainSection = true;
            continue;
        }
        if (!inMainSection)
            continue;
        const int eq = line.indexOf(QLatin1Char('='));
        if (eq <= 0)
            continue;
        const QString key = line.left(eq).trimmed();
        const QString value = line.mid(eq + 1).trimmed();
        if (key.contains(QLatin1Char('[')))
            localeEntries.append({ key, value });
        else
            values.insert(key, value);
    }

    if (!sawMainSection || !values.contains(QStringLiteral("Exec")))
        return std::nullopt;

    const QString type = values.value(QStringLiteral("Type"), QStringLiteral("Application"));
    if (type != QLatin1String("Application"))
        return std::nullopt;

    const QString exec = values.value(QStringLiteral("Exec"));
    if (exec.trimmed().isEmpty())
        return std::nullopt;

    DesktopApp app;
    app.id = desktopId;
    app.path = path;
    app.exec = exec;

    // Locale fallback: bare key wins, else first localized variant.
    auto readWithLocale = [&](const QString& base) -> std::optional<QString> {
        if (values.contains(base))
            return values.value(base);
        const QString prefix = base + QLatin1Char('[');
        for (const auto& [key, value] : localeEntries) {
            if (key.startsWith(prefix))
                return value;
        }
        return std::nullopt;
    };

    app.name = readWithLocale(QStringLiteral("Name")).value_or(desktopId);
    app.genericName = readWithLocale(QStringLiteral("GenericName"));
    app.comment = readWithLocale(QStringLiteral("Comment"));
    app.icon = readWithLocale(QStringLiteral("Icon"));

    const auto splitList = [](const QString& raw) {
        QStringList out;
        for (const QString& part : raw.split(QLatin1Char(';')))
            if (!part.trimmed().isEmpty())
                out << part.trimmed();
        return out;
    };
    if (values.contains(QStringLiteral("Categories")))
        app.categories = splitList(values.value(QStringLiteral("Categories")));
    if (values.contains(QStringLiteral("Keywords")))
        app.keywords = splitList(values.value(QStringLiteral("Keywords")));

    app.terminal =
        values.value(QStringLiteral("Terminal")).compare(QLatin1String("true"), Qt::CaseInsensitive) == 0;
    app.noDisplay =
        values.value(QStringLiteral("NoDisplay")).compare(QLatin1String("true"), Qt::CaseInsensitive) == 0;

    if (app.noDisplay)
        return std::nullopt;

    return app;
}

std::optional<QString> execBinaryToken(const QString& execLine)
{
    QString s = execLine.trimmed();

    // Strip leading repeated `env VAR=VALUE` prefixes.
    for (;;) {
        QStringList tokens;
        QString current;
        bool inDouble = false;
        bool inSingle = false;
        bool hasToken = false;
        for (const QChar ch : std::as_const(s)) {
            if (inDouble) {
                if (ch == QLatin1Char('"')) {
                    inDouble = false;
                } else {
                    current += ch;
                }
            } else if (inSingle) {
                if (ch == QLatin1Char('\'')) {
                    inSingle = false;
                } else {
                    current += ch;
                }
            } else if (ch == QLatin1Char('"')) {
                inDouble = true;
                hasToken = true;
            } else if (ch == QLatin1Char('\'')) {
                inSingle = true;
                hasToken = true;
            } else if (ch.isSpace()) {
                if (hasToken || !current.isEmpty()) {
                    tokens << current;
                    current.clear();
                    hasToken = false;
                }
            } else {
                current += ch;
                hasToken = true;
            }
        }
        if (hasToken || !current.isEmpty())
            tokens << current;

        if (tokens.isEmpty())
            return std::nullopt;
        if (tokens.first() == QLatin1String("env") && tokens.size() >= 2 &&
            tokens.at(1).contains(QLatin1Char('='))) {
            // Drop "env" and all VAR=VALUE tokens, then re-tokenize the rest.
            int drop = 1;
            while (drop < tokens.size() && tokens.at(drop).contains(QLatin1Char('=')))
                ++drop;
            s = QStringList(tokens.mid(drop)).join(QLatin1Char(' '));
            continue;
        }

        for (const QString& token : tokens) {
            if (!isFieldCode(token))
                return token;
        }
        return std::nullopt; // only field codes present
    }
}

std::optional<QString> resolveExecBinary(const QString& token)
{
    if (token.isEmpty())
        return std::nullopt;

    QFileInfo info(token);
    if (info.isAbsolute()) {
        if (info.isFile())
            return info.absoluteFilePath();
        // Try one symlink/canonicalization level.
        const QString canonical = info.canonicalFilePath();
        if (!canonical.isEmpty() && QFileInfo(canonical).isFile())
            return canonical;
        return std::nullopt;
    }

    const QStringList dirs = qEnvironmentVariable("PATH")
                                 .split(QLatin1Char(':'), Qt::SkipEmptyParts);
    for (const QString& dir : dirs) {
        const QFileInfo candidate(dir + QLatin1Char('/') + token);
        if (candidate.isFile())
            return candidate.absoluteFilePath();
    }
    return std::nullopt;
}

DesktopIndex::DesktopIndex(QList<DesktopApp> apps)
{
    build(std::move(apps));
}

void DesktopIndex::build(QList<DesktopApp> apps)
{
    m_apps = std::move(apps);
    m_byId.clear();
    m_byExec.clear();
    m_byNameLower.clear();
    m_matched.clear();

    std::sort(m_apps.begin(), m_apps.end(), [](const DesktopApp& a, const DesktopApp& b) {
        return a.name.compare(b.name, Qt::CaseInsensitive) < 0;
    });

    for (int i = 0; i < m_apps.size(); ++i) {
        const DesktopApp& app = m_apps.at(i);
        m_byId.insert(app.id.toLower(), i);

        const std::optional<QString> binary = execBinaryToken(app.exec);
        if (binary) {
            const QString base = QFileInfo(*binary).fileName().toLower();
            if (!base.isEmpty() && !m_byExec.contains(base))
                m_byExec.insert(base, i);
        }
        const QString nameLower = app.name.toLower();
        if (!m_byNameLower.contains(nameLower))
            m_byNameLower.insert(nameLower, i);
    }
}

std::optional<int> DesktopIndex::findOnly(const QString& id) const
{
    const auto it = m_byId.constFind(id.toLower());
    if (it == m_byId.cend())
        return std::nullopt;
    return it.value();
}

std::optional<int> DesktopIndex::findExec(const QString& execBasename) const
{
    const auto it = m_byExec.constFind(execBasename.toLower());
    if (it == m_byExec.cend())
        return std::nullopt;
    return it.value();
}

std::optional<int> DesktopIndex::findName(const QString& nameLower) const
{
    const auto it = m_byNameLower.constFind(nameLower.toLower());
    if (it == m_byNameLower.cend())
        return std::nullopt;
    return it.value();
}

std::optional<DesktopApp> DesktopIndex::takeAt(std::optional<int> index)
{
    if (!index || *index < 0 || *index >= m_apps.size())
        return std::nullopt;
    m_matched.insert(*index);
    return m_apps.at(*index);
}

std::optional<DesktopApp> DesktopIndex::lookupFlatpak(const QString& appId)
{
    return takeAt(findOnly(appId));
}

std::optional<DesktopApp> DesktopIndex::lookupSnap(const QString& snapName)
{
    // ids starting "<snapName>_" or equal to snapName.
    const QString prefix = snapName + QLatin1Char('_');
    for (auto it = m_byId.cbegin(); it != m_byId.cend(); ++it) {
        const QString key = it.key();
        if (key == snapName.toLower() || key.startsWith(prefix.toLower()))
            return takeAt(it.value());
    }
    if (auto byId = findOnly(snapName))
        return takeAt(byId);
    if (auto byExec = findExec(snapName))
        return takeAt(byExec);
    return std::nullopt;
}

std::optional<DesktopApp> DesktopIndex::lookupGeneric(const QString& id,
                                                      const QString& displayName)
{
    if (auto hit = findOnly(id))
        return takeAt(hit);

    const QString base = QFileInfo(id).fileName();
    if (auto hit = findOnly(base))
        return takeAt(hit);

    QString stem = base;
    if (stem.endsWith(QLatin1String(".appimage"), Qt::CaseInsensitive))
        stem.chop(QStringLiteral(".appimage").size());
    if (auto hit = findOnly(stem))
        return takeAt(hit);

    // Common packaging-suffix fallback: APT packages like "helium-bin" ship a
    // desktop entry named "helium" (Icon=helium, Exec=helium).
    QString stripped = id.toLower();
    for (const char* suffix : { "-bin", "_bin" }) {
        const QString s = QString::fromLatin1(suffix);
        if (stripped.endsWith(s) && stripped.size() > s.size()) {
            stripped.chop(s.size());
            break;
        }
    }
    if (!stripped.isEmpty() && stripped != id.toLower()) {
        if (auto hit = findOnly(stripped))
            return takeAt(hit);
        if (auto hit = findExec(stripped))
            return takeAt(hit);
    }

    if (auto hit = findExec(base))
        return takeAt(hit);
    if (auto hit = findExec(stem))
        return takeAt(hit);

    if (!displayName.isEmpty()) {
        if (auto hit = findName(displayName))
            return takeAt(hit);
    }
    return std::nullopt;
}

QList<DesktopApp> DesktopIndex::unmatched() const
{
    QList<DesktopApp> out;
    for (int i = 0; i < m_apps.size(); ++i) {
        if (!m_matched.contains(i))
            out << m_apps.at(i);
    }
    return out;
}

QList<DesktopApp> DesktopIndex::collect()
{
    struct Root {
        QString path;
    };
    QList<Root> roots;
    QSet<QString> seenRoots;
    for (const QString& dir : applicationDirs()) {
        if (QDir(dir).exists() && !seenRoots.contains(dir)) {
            seenRoots.insert(dir);
            roots.append({ dir });
        }
    }

    QHash<QString, DesktopApp> byId;
    for (const Root& root : roots) {
        QDirIterator it(root.path,
                        { QStringLiteral("*.desktop") }, QDir::Files,
                        QDirIterator::Subdirectories);
        while (it.hasNext()) {
            const QString filePath = it.next();
            bool blacklisted = false;
            for (const QString& bad : blacklistedDirSubstrings()) {
                if (filePath.contains(bad)) {
                    blacklisted = true;
                    break;
                }
            }
            if (blacklisted)
                continue;

            const QString id = desktopIdFor(root.path, filePath);
            if (byId.contains(id))
                continue;

            auto app = parseDesktopFile(filePath, id);
            if (app)
                byId.insert(id, std::move(*app));
        }
    }

    QList<DesktopApp> apps = byId.values();
    std::sort(apps.begin(), apps.end(), [](const DesktopApp& a, const DesktopApp& b) {
        return a.name.compare(b.name, Qt::CaseInsensitive) < 0;
    });
    return apps;
}

} // namespace scope
