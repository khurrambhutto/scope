#pragma once

#include "models/PackageListModel.h"
#include "scanner/ScanService.h"

#include <QObject>

namespace scope {

// Top-level controller for the unified package surface: owns the scan service
// and the filtered list model exposed to QML.
class PackagesController : public QObject
{
    Q_OBJECT
    Q_PROPERTY(PackageListModel* packages READ packages CONSTANT)
    Q_PROPERTY(bool scanning READ scanning NOTIFY scanningChanged)
    Q_PROPERTY(QVariantMap availability READ availability NOTIFY availabilityChanged)
    Q_PROPERTY(QString lastScanText READ lastScanText NOTIFY availabilityChanged)
    Q_PROPERTY(QString error READ error NOTIFY availabilityChanged)
    Q_PROPERTY(int updateCount READ updateCount NOTIFY availabilityChanged)

public:
    explicit PackagesController(QObject* parent = nullptr);

    [[nodiscard]] PackageListModel* packages() { return &m_model; }
    [[nodiscard]] bool scanning() const { return m_service.isScanning(); }
    [[nodiscard]] QVariantMap availability() const;
    [[nodiscard]] QString lastScanText() const;
    [[nodiscard]] QString error() const { return m_error; }
    [[nodiscard]] int updateCount() const { return m_model.updateCount(); }

    Q_INVOKABLE void refresh();
    Q_INVOKABLE void setQuery(const QString& query) { m_model.setQuery(query); }
    Q_INVOKABLE void setSourceFilter(const QString& source) { m_model.setSourceFilter(source); }
    Q_INVOKABLE void setKindFilter(const QString& kind) { m_model.setKindFilter(kind); }

signals:
    void scanningChanged();
    void availabilityChanged();

private:
    void onScanFinished(const ScanResult& result);

    ScanService m_service;
    PackageListModel m_model;
    CachedScan m_scan;
    QString m_error;
};

} // namespace scope
