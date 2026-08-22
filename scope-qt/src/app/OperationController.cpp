#include "OperationController.h"

#include "app/PackagesController.h"
#include "operations/UninstallOp.h"
#include "operations/UpdateOp.h"
#include "scanner/ScanService.h"

#include <QMetaObject>
#include <QThreadPool>

namespace scope {

OperationController::OperationController(PackagesController* packages, QObject* parent)
    : QObject(parent)
    , m_packages(packages)
{
}

void OperationController::previewUninstall(const QString& packageKey)
{
    handlePreview(packageKey, Operation::Uninstall);
}

void OperationController::previewUpdate(const QString& packageKey)
{
    handlePreview(packageKey, Operation::Update);
}

void OperationController::handlePreview(const QString& packageKey, Operation operation)
{
    const InstalledPackage* pkg =
        m_packages ? m_packages->packages()->findByKey(packageKey) : nullptr;
    if (!pkg) {
        emit previewFailed(QStringLiteral("Package not found in current scan: %1").arg(packageKey));
        return;
    }

    if (operation == Operation::Update && !pkg->hasUpdate) {
        emit previewFailed(QStringLiteral("'%1' has no updates available.").arg(pkg->effectiveName()));
        return;
    }

    OperationPlan plan = operation == Operation::Uninstall ? uninstall::preview(*pkg)
                                                           : update::preview(*pkg);

    // Protected plans are returned for display but never issued — their id can
    // never be applied.
    if (!plan.isProtected) {
        plan.planId = newPlanId(m_plans.nextCounter());
        m_plans.issue(plan);
    }

    emit previewReady(plan.toVariantMap());
}

void OperationController::applyUninstall(const QString& planId)
{
    auto plan = m_plans.take(planId);
    if (!plan) {
        emit previewFailed(
            QStringLiteral("Stale or unknown uninstall plan. Please preview again."));
        return;
    }
    runApply(std::move(*plan), Operation::Uninstall);
}

void OperationController::applyUpdate(const QString& planId)
{
    auto plan = m_plans.take(planId);
    if (!plan) {
        emit previewFailed(QStringLiteral("Stale or unknown update plan. Please preview again."));
        return;
    }
    runApply(std::move(*plan), Operation::Update);
}

void OperationController::runApply(OperationPlan plan, Operation operation)
{
    const QString title =
        QStringLiteral("%1 %2")
            .arg(operation == Operation::Uninstall ? QStringLiteral("Uninstalling")
                                                   : QStringLiteral("Updating"),
                 plan.displayName);
    const int taskId = m_tasks.beginTask(title);

    // All heavy work happens off the UI thread; results marshal back queued.
    QThreadPool::globalInstance()->start([this, taskId, plan = std::move(plan),
                                          operation]() mutable {
        ScanService scanner;
        const ScanResult fresh = scanner.scanBlocking();

        QString error;
        if (!fresh.ok)
            error = QStringLiteral("Rescan failed before applying: %1").arg(fresh.error);
        else if (operation == Operation::Uninstall)
            error = uninstall::revalidate(plan, fresh.scan);
        else
            error = update::revalidate(plan, fresh.scan);

        OperationResult result;
        if (!error.isEmpty()) {
            result.success = false;
            result.message = error;
        } else {
            result = operation == Operation::Uninstall ? uninstall::apply(plan)
                                                       : update::apply(plan);
        }

        const bool success = result.success;
        const QString message = result.message;
        const QString logs = result.logs;

        QMetaObject::invokeMethod(
            this,
            [this, taskId, success, message, logs]() {
                m_tasks.finishTask(taskId, success, message, logs);
                if (success && m_packages)
                    m_packages->refresh();
            },
            Qt::QueuedConnection);
    });
}

} // namespace scope
