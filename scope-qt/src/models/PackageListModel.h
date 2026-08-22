#pragma once

#include "package/Package.h"
#include "safety/Safety.h"

#include <QAbstractListModel>
#include <QHash>

namespace scope {

// Virtualized list model over the unified package scan with in-model
// search/source/kind filtering (no re-invoking the backend per keystroke).
class PackageListModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(int totalCount READ totalCount NOTIFY totalCountChanged)
    Q_PROPERTY(int updateCount READ updateCount NOTIFY totalCountChanged)

public:
    enum Roles {
        KeyRole = Qt::UserRole + 1,
        NameRole,
        PackageIdRole,
        SourceRole,
        SourceLabelRole,
        VersionRole,
        UpdateVersionRole,
        HasUpdateRole,
        SizeTextRole,
        AppKindRole,
        IconUrlRole,
        DescriptionRole,
        CategoriesRole,
        TerminalRole,
        ProtectedRole,
        ProtectionReasonRole,
        InstallScopeRole,
    };

    explicit PackageListModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    void setPackages(const QList<InstalledPackage>& packages);

    void setQuery(const QString& query);
    void setSourceFilter(const QString& sourceId);
    void setKindFilter(const QString& kindId);

    [[nodiscard]] int totalCount() const { return m_packages.size(); }
    [[nodiscard]] int updateCount() const;

    [[nodiscard]] const InstalledPackage* packageAtRow(int row) const;
    [[nodiscard]] const InstalledPackage* findByKey(const QString& key) const;

    // Full row payload for the detail drawer.
    Q_INVOKABLE QVariantMap packageData(int row) const;

signals:
    void totalCountChanged();

private:
    void refilter();

    QList<InstalledPackage> m_packages;
    QVector<const InstalledPackage*> m_visible;

    QString m_query;
    std::optional<PackageSource> m_sourceFilter;
    std::optional<AppKind> m_kindFilter;

    mutable QHash<QString, Protection> m_protectionCache;
};

[[nodiscard]] QString formatSize(quint64 bytes);

} // namespace scope
