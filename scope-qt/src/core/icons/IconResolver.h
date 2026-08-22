#pragma once

#include <QHash>
#include <QSet>
#include <QString>

#include <optional>

namespace scope {

// Resolves icon names to real files using the freedesktop icon theme spec,
// with a process-wide memo cache (negative results included).
class IconResolver
{
public:
    static IconResolver& instance();

    // Raw `Icon=` value or file name -> absolute file path (memoized).
    [[nodiscard]] std::optional<QString> resolve(const QString& iconName);

    // Registers a resolved path for serving and returns an image-provider URL
    // ("image://scopeicon/<percent-encoded path>"). Empty string when the path
    // is not a valid icon candidate.
    [[nodiscard]] QString iconUrl(const QString& resolvedPath);

    // Whitelist check used by the image provider. Input may be any path; it is
    // canonicalized before membership test.
    [[nodiscard]] static bool isRegisteredPath(const QString& path);

    [[nodiscard]] static bool isIconCandidate(const QString& path);

private:
    IconResolver() = default;

    std::optional<QString> resolveUncached(const QString& iconName);
    std::optional<QString> lookupInTheme(const QString& theme, const QString& name,
                                         QHash<QString, bool>& parentCache) const;
    std::optional<QString> lookupInPixmaps(const QString& name) const;
    QString activeTheme() const;

    QHash<QString, std::optional<QString>> m_cache;
    QSet<QString> m_servedPaths;
};

} // namespace scope

namespace scope::iconurl {
[[nodiscard]] QString encode(const QString& path);
[[nodiscard]] QString decode(const QString& encoded);
} // namespace scope::iconurl
