#include "PackageListModel.h"

#include <algorithm>

namespace scope {

PackageListModel::PackageListModel(QObject* parent)
    : QAbstractListModel(parent)
{
}

int PackageListModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : m_visible.size();
}

QHash<int, QByteArray> PackageListModel::roleNames() const
{
    QHash<int, QByteArray> roles;
    roles[KeyRole] = "key";
    roles[NameRole] = "name";
    roles[PackageIdRole] = "packageId";
    roles[SourceRole] = "source";
    roles[SourceLabelRole] = "sourceLabel";
    roles[VersionRole] = "version";
    roles[UpdateVersionRole] = "updateVersion";
    roles[HasUpdateRole] = "hasUpdate";
    roles[SizeTextRole] = "sizeText";
    roles[AppKindRole] = "appKind";
    roles[IconUrlRole] = "iconUrl";
    roles[DescriptionRole] = "description";
    roles[CategoriesRole] = "categories";
    roles[TerminalRole] = "terminal";
    roles[ProtectedRole] = "isProtected";
    roles[ProtectionReasonRole] = "protectionReason";
    roles[InstallScopeRole] = "installScope";
    return roles;
}

QVariant PackageListModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() >= m_visible.size())
        return {};
    const InstalledPackage& pkg = *m_visible.at(index.row());

    switch (role) {
    case KeyRole:
        return pkg.key;
    case NameRole:
        return pkg.effectiveName();
    case PackageIdRole:
        return pkg.packageId;
    case SourceRole:
        return sourceId(pkg.source);
    case SourceLabelRole:
        return sourceLabel(pkg.source);
    case VersionRole:
        return pkg.version.isEmpty() ? QStringLiteral("—") : pkg.version;
    case UpdateVersionRole:
        return pkg.updateVersion.value_or(QString());
    case HasUpdateRole:
        return pkg.hasUpdate;
    case SizeTextRole:
        return formatSize(pkg.sizeBytes);
    case AppKindRole:
        return appKindId(pkg.appKind);
    case IconUrlRole:
        return pkg.icon.value_or(QString());
    case DescriptionRole:
        return pkg.description.value_or(QString());
    case CategoriesRole:
        return pkg.categories.value_or(QString());
    case TerminalRole:
        return pkg.terminal;
    case ProtectedRole: {
        auto& cache = const_cast<QHash<QString, Protection>&>(m_protectionCache);
        auto it = cache.constFind(pkg.key);
        if (it == cache.cend())
            it = cache.insert(pkg.key, checkPackage(pkg.source, pkg.packageId));
        return it->isProtected;
    }
    case ProtectionReasonRole: {
        auto& cache = const_cast<QHash<QString, Protection>&>(m_protectionCache);
        auto it = cache.constFind(pkg.key);
        if (it == cache.cend())
            it = cache.insert(pkg.key, checkPackage(pkg.source, pkg.packageId));
        return it->reason;
    }
    case InstallScopeRole:
        return pkg.installScope ? scopeId(*pkg.installScope) : QString();
    }
    return {};
}

void PackageListModel::setPackages(const QList<InstalledPackage>& packages)
{
    beginResetModel();
    m_packages = packages;
    m_protectionCache.clear();
    refilter();
    endResetModel();
    emit totalCountChanged();
}

void PackageListModel::setQuery(const QString& query)
{
    if (m_query == query)
        return;
    beginResetModel();
    m_query = query;
    refilter();
    endResetModel();
}

void PackageListModel::setSourceFilter(const QString& sourceIdStr)
{
    std::optional<PackageSource> parsed =
        sourceIdStr.isEmpty() ? std::nullopt : parseSource(sourceIdStr);
    if (m_sourceFilter == parsed)
        return;
    beginResetModel();
    m_sourceFilter = parsed;
    refilter();
    endResetModel();
}

void PackageListModel::setKindFilter(const QString& kindId)
{
    std::optional<AppKind> kind;
    if (kindId == QLatin1String("gui"))
        kind = AppKind::Gui;
    else if (kindId == QLatin1String("cli"))
        kind = AppKind::Cli;
    else if (kindId == QLatin1String("unknown"))
        kind = AppKind::Unknown;

    if (m_kindFilter == kind)
        return;
    beginResetModel();
    m_kindFilter = kind;
    refilter();
    endResetModel();
}

int PackageListModel::updateCount() const
{
    int count = 0;
    for (const InstalledPackage& pkg : m_packages) {
        if (pkg.hasUpdate)
            ++count;
    }
    return count;
}

const InstalledPackage* PackageListModel::packageAtRow(int row) const
{
    if (row < 0 || row >= m_visible.size())
        return nullptr;
    return m_visible.at(row);
}

const InstalledPackage* PackageListModel::findByKey(const QString& key) const
{
    for (const InstalledPackage& pkg : m_packages) {
        if (pkg.key == key)
            return &pkg;
    }
    return nullptr;
}

QVariantMap PackageListModel::packageData(int row) const
{
    const InstalledPackage* pkg = packageAtRow(row);
    if (!pkg)
        return {};
    QVariantMap map = pkg->toVariantMap();

    const Protection protection = checkPackage(pkg->source, pkg->packageId);

    // QML-friendly camelCase view of the same payload.
    map.insert(QStringLiteral("displayName"), pkg->effectiveName());
    map.insert(QStringLiteral("packageId"), pkg->packageId);
    map.insert(QStringLiteral("sourceLabel"), sourceLabel(pkg->source));
    map.insert(QStringLiteral("sizeText"), formatSize(pkg->sizeBytes));
    map.insert(QStringLiteral("appKind"), appKindId(pkg->appKind));
    map.insert(QStringLiteral("iconUrl"), pkg->icon.value_or(QString()));
    map.insert(QStringLiteral("hasUpdate"), pkg->hasUpdate);
    map.insert(QStringLiteral("updateVersion"), pkg->updateVersion.value_or(QString()));
    map.insert(QStringLiteral("isProtected"), protection.isProtected);
    map.insert(QStringLiteral("protectionReason"), protection.reason);
    map.insert(QStringLiteral("installScope"),
               pkg->installScope ? scopeId(*pkg->installScope) : QString());
    return map;
}

void PackageListModel::refilter()
{
    m_visible.clear();
    m_visible.reserve(m_packages.size());

    const QString needle = m_query.trimmed().toLower();

    for (const InstalledPackage& pkg : m_packages) {
        if (m_sourceFilter && pkg.source != *m_sourceFilter)
            continue;
        if (m_kindFilter && pkg.appKind != *m_kindFilter)
            continue;

        if (!needle.isEmpty()) {
            QString haystack = pkg.name.toLower();
            if (pkg.displayName)
                haystack += QLatin1Char(' ') + pkg.displayName->toLower();
            if (pkg.description)
                haystack += QLatin1Char(' ') + pkg.description->toLower();
            haystack += QLatin1Char(' ') + pkg.packageId.toLower();
            if (pkg.installScope)
                haystack += QLatin1Char(' ') + scopeId(*pkg.installScope);
            if (pkg.categories)
                haystack += QLatin1Char(' ') + pkg.categories->toLower();
            haystack += QLatin1Char(' ') + pkg.version.toLower();
            if (!haystack.contains(needle))
                continue;
        }

        m_visible.append(&pkg);
    }
}

} // namespace scope
