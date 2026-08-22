#include "PackagesController.h"

#include <QDateTime>

namespace scope {

PackagesController::PackagesController(QObject* parent)
    : QObject(parent)
{
    qRegisterMetaType<scope::ScanResult>("scope::ScanResult");

    connect(&m_service, &ScanService::scanStarted, this, [this]() {
        emit scanningChanged();
    });
    connect(&m_service, &ScanService::scanFinished, this, &PackagesController::onScanFinished);
}

void PackagesController::refresh()
{
    m_error.clear();
    m_service.scanAllAsync();
}

void PackagesController::onScanFinished(const ScanResult& result)
{
    if (!result.ok) {
        m_error = result.error;
    } else {
        m_error.clear();
        m_scan = result.scan;
        m_model.setPackages(m_scan.packages);
    }
    emit scanningChanged();
    emit availabilityChanged();
}

QVariantMap PackagesController::availability() const
{
    return m_scan.availability.toVariantMap();
}

QString PackagesController::lastScanText() const
{
    if (m_scan.scannedAtMs == 0)
        return QStringLiteral("never");
    return QDateTime::fromMSecsSinceEpoch(m_scan.scannedAtMs)
        .toString(QStringLiteral("hh:mm:ss"));
}

} // namespace scope
