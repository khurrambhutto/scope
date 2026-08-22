#include "Package.h"

#include <QHash>

namespace scope {

QString sourceId(PackageSource source)
{
    switch (source) {
    case PackageSource::Apt: return QStringLiteral("apt");
    case PackageSource::Snap: return QStringLiteral("snap");
    case PackageSource::Flatpak: return QStringLiteral("flatpak");
    case PackageSource::AppImage: return QStringLiteral("appimage");
    case PackageSource::Manual: return QStringLiteral("manual");
    }
    return QStringLiteral("apt");
}

QString sourceLabel(PackageSource source)
{
    switch (source) {
    case PackageSource::Apt: return QStringLiteral("APT");
    case PackageSource::Snap: return QStringLiteral("Snap");
    case PackageSource::Flatpak: return QStringLiteral("Flatpak");
    case PackageSource::AppImage: return QStringLiteral("AppImage");
    case PackageSource::Manual: return QStringLiteral("Manual");
    }
    return QStringLiteral("APT");
}

std::optional<PackageSource> parseSource(const QString& id)
{
    static const QHash<QString, PackageSource> map = {
        { QStringLiteral("apt"), PackageSource::Apt },
        { QStringLiteral("snap"), PackageSource::Snap },
        { QStringLiteral("flatpak"), PackageSource::Flatpak },
        { QStringLiteral("appimage"), PackageSource::AppImage },
        { QStringLiteral("manual"), PackageSource::Manual },
    };
    const auto it = map.constFind(id);
    if (it == map.cend())
        return std::nullopt;
    return it.value();
}

QString scopeId(InstallScope scope)
{
    return scope == InstallScope::User ? QStringLiteral("user") : QStringLiteral("system");
}

std::optional<InstallScope> parseScope(const QString& id)
{
    if (id == QLatin1String("user"))
        return InstallScope::User;
    if (id == QLatin1String("system"))
        return InstallScope::System;
    return std::nullopt;
}

QString appKindId(AppKind kind)
{
    switch (kind) {
    case AppKind::Gui: return QStringLiteral("gui");
    case AppKind::Cli: return QStringLiteral("cli");
    case AppKind::Unknown: return QStringLiteral("unknown");
    }
    return QStringLiteral("unknown");
}

int appKindRank(AppKind kind)
{
    switch (kind) {
    case AppKind::Gui: return 0;
    case AppKind::Cli: return 1;
    case AppKind::Unknown: return 2;
    }
    return 2;
}

QString makeKey(PackageSource source, const QString& packageId)
{
    return sourceId(source) + QLatin1Char(':') + packageId;
}

QString makeKey(PackageSource source, const QString& packageId, InstallScope scope)
{
    return sourceId(source) + QLatin1Char(':') + scopeId(scope) + QLatin1Char(':') + packageId;
}

QString formatSize(quint64 bytes)
{
    if (bytes == 0)
        return QStringLiteral("—");
    static const char* units[] = { "B", "KB", "MB", "GB", "TB" };
    double value = static_cast<double>(bytes);
    int unit = 0;
    while (value >= 1024.0 && unit < 4) {
        value /= 1024.0;
        ++unit;
    }
    return unit == 0
        ? QStringLiteral("%1 %2").arg(static_cast<quint64>(value)).arg(QLatin1String(units[unit]))
        : QStringLiteral("%1 %2").arg(value, 0, 'f', 1).arg(QLatin1String(units[unit]));
}

QVariantMap InstalledPackage::toVariantMap() const
{
    QVariantMap map;
    map.insert(QStringLiteral("key"), key);
    map.insert(QStringLiteral("source"), sourceId(source));
    map.insert(QStringLiteral("package_id"), packageId);
    if (installScope)
        map.insert(QStringLiteral("install_scope"), scopeId(*installScope));
    map.insert(QStringLiteral("name"), name);
    if (displayName)
        map.insert(QStringLiteral("display_name"), *displayName);
    if (description)
        map.insert(QStringLiteral("description"), *description);
    map.insert(QStringLiteral("version"), version);
    map.insert(QStringLiteral("size_bytes"), QString::number(sizeBytes));
    map.insert(QStringLiteral("app_kind"), appKindId(appKind));
    if (icon)
        map.insert(QStringLiteral("icon"), *icon);
    if (categories)
        map.insert(QStringLiteral("categories"), *categories);
    map.insert(QStringLiteral("terminal"), terminal);
    map.insert(QStringLiteral("has_update"), hasUpdate);
    if (updateVersion)
        map.insert(QStringLiteral("update_version"), *updateVersion);
    return map;
}

QVariantMap ScanAvailability::toVariantMap() const
{
    QVariantMap map;
    map.insert(QStringLiteral("apt"), apt);
    map.insert(QStringLiteral("snap"), snap);
    map.insert(QStringLiteral("flatpak"), flatpak);
    map.insert(QStringLiteral("appimage"), appimage);
    map.insert(QStringLiteral("manual"), manual);
    if (!aptError.isEmpty())
        map.insert(QStringLiteral("apt_error"), aptError);
    if (!snapError.isEmpty())
        map.insert(QStringLiteral("snap_error"), snapError);
    if (!flatpakError.isEmpty())
        map.insert(QStringLiteral("flatpak_error"), flatpakError);
    map.insert(QStringLiteral("appimage_dirs"), appimageDirs);
    return map;
}

const InstalledPackage* CachedScan::findByKey(const QString& searchKey) const
{
    for (const InstalledPackage& pkg : packages) {
        if (pkg.key == searchKey)
            return &pkg;
    }
    return nullptr;
}

} // namespace scope
