#pragma once

#include <QHash>
#include <QList>
#include <QSet>
#include <QString>
#include <QStringList>

#include <optional>

namespace scope {

struct DesktopApp {
    QString id;
    QString path;
    QString name;
    QString exec;
    std::optional<QString> genericName;
    std::optional<QString> comment;
    std::optional<QString> icon;
    QStringList categories;
    QStringList keywords;
    bool terminal = false;
    bool noDisplay = false;
};

// Discovers and indexes visible .desktop applications.
class DesktopIndex
{
public:
    // Walks all application dirs and returns parsed, sorted entries.
    [[nodiscard]] static QList<DesktopApp> collect();

    DesktopIndex() = default;
    explicit DesktopIndex(QList<DesktopApp> apps);

    void build(QList<DesktopApp> apps);

    // Source-specific lookup rules; marks the returned entry as matched.
    [[nodiscard]] std::optional<DesktopApp> lookupFlatpak(const QString& appId);
    [[nodiscard]] std::optional<DesktopApp> lookupSnap(const QString& snapName);
    [[nodiscard]] std::optional<DesktopApp> lookupGeneric(const QString& id,
                                                          const QString& displayName);

    [[nodiscard]] QList<DesktopApp> unmatched() const;

private:
    QList<DesktopApp> m_apps;
    QHash<QString, int> m_byId;        // lowercased desktop id -> index
    QHash<QString, int> m_byExec;      // lowercased exec basename -> index
    QHash<QString, int> m_byNameLower; // lowercased display name -> index
    QSet<int> m_matched;

    std::optional<int> findOnly(const QString& id) const;          // byId, no marking
    std::optional<int> findExec(const QString& execBasename) const; // byExec, no marking
    std::optional<int> findName(const QString& nameLower) const;    // byName, no marking

    std::optional<DesktopApp> takeAt(std::optional<int> index);
};

// Parses a single .desktop file. Returns nullopt when invalid/hidden/not an app.
[[nodiscard]] std::optional<DesktopApp> parseDesktopFile(const QString& path,
                                                         const QString& desktopId);

// Extracts the first real binary token from an Exec= line.
[[nodiscard]] std::optional<QString> execBinaryToken(const QString& execLine);

// Resolves an Exec token to an existing file path when possible.
[[nodiscard]] std::optional<QString> resolveExecBinary(const QString& token);

} // namespace scope
