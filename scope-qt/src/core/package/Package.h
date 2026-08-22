#pragma once

#include <QString>
#include <QStringList>
#include <QVariantMap>
#include <QtGlobal>

#include <optional>

namespace scope {

enum class PackageSource {
    Apt,
    Snap,
    Flatpak,
    AppImage,
    Manual,
};

enum class AppKind {
    Unknown,
    Gui,
    Cli,
};

enum class InstallScope {
    User,
    System,
};

[[nodiscard]] QString sourceId(PackageSource source);
[[nodiscard]] QString sourceLabel(PackageSource source);
[[nodiscard]] std::optional<PackageSource> parseSource(const QString& id);

[[nodiscard]] QString scopeId(InstallScope scope);
[[nodiscard]] std::optional<InstallScope> parseScope(const QString& id);

[[nodiscard]] QString appKindId(AppKind kind);
[[nodiscard]] int appKindRank(AppKind kind);

[[nodiscard]] QString makeKey(PackageSource source, const QString& packageId);
[[nodiscard]] QString makeKey(PackageSource source, const QString& packageId, InstallScope scope);

[[nodiscard]] QString formatSize(quint64 bytes);

struct InstalledPackage {
    QString key;
    PackageSource source = PackageSource::Apt;
    QString packageId;
    std::optional<InstallScope> installScope;
    QString name;
    std::optional<QString> displayName;
    std::optional<QString> description;
    QString version;
    quint64 sizeBytes = 0;
    AppKind appKind = AppKind::Unknown;
    std::optional<QString> icon;
    std::optional<QString> categories;
    bool terminal = false;
    bool hasUpdate = false;
    std::optional<QString> updateVersion;

    [[nodiscard]] QString effectiveName() const
    {
        return displayName && !displayName->isEmpty() ? *displayName : name;
    }

    [[nodiscard]] QVariantMap toVariantMap() const;
};

struct ScanAvailability {
    bool apt = false;
    bool snap = false;
    bool flatpak = false;
    bool appimage = false;
    bool manual = false;
    QString aptError;
    QString snapError;
    QString flatpakError;
    QStringList appimageDirs;

    [[nodiscard]] QVariantMap toVariantMap() const;
};

struct CachedScan {
    QList<InstalledPackage> packages;
    ScanAvailability availability;
    qint64 scannedAtMs = 0;

    [[nodiscard]] const InstalledPackage* findByKey(const QString& key) const;
};

} // namespace scope
