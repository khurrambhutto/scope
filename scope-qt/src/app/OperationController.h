#pragma once

#include "app/TaskModel.h"
#include "operations/PlanStore.h"
#include "package/Package.h"

#include <QObject>

namespace scope {

class PackagesController;

// Preview + apply orchestration for uninstall and update. Applies never block
// the UI: confirm closes the dialog, the task runs in the background, and a
// successful operation triggers an automatic rescan.
class OperationController : public QObject
{
    Q_OBJECT
    Q_PROPERTY(TaskModel* tasks READ tasks CONSTANT)

public:
    explicit OperationController(PackagesController* packages, QObject* parent = nullptr);

    [[nodiscard]] TaskModel* tasks() { return &m_tasks; }

    Q_INVOKABLE void previewUninstall(const QString& packageKey);
    Q_INVOKABLE void applyUninstall(const QString& planId);
    Q_INVOKABLE void previewUpdate(const QString& packageKey);
    Q_INVOKABLE void applyUpdate(const QString& planId);

signals:
    void previewReady(const QVariantMap& plan);
    void previewFailed(const QString& message);

private:
    void handlePreview(const QString& packageKey, Operation operation);
    void runApply(OperationPlan plan, Operation operation);

    PackagesController* m_packages = nullptr;
    PlanStore m_plans;
    TaskModel m_tasks;
};

} // namespace scope
