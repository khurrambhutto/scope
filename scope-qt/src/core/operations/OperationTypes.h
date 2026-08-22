#pragma once

#include "package/Package.h"
#include "system/System.h"

#include <QString>
#include <QVector>

#include <optional>

namespace scope {

enum class Operation {
    Uninstall,
    Update,
};

[[nodiscard]] inline QString operationId(Operation op)
{
    return op == Operation::Uninstall ? QStringLiteral("uninstall") : QStringLiteral("update");
}

struct PlanStep {
    QString description;
    QString commandSummary; // display-only; never re-parsed
};

struct OperationPlan {
    QString planId;
    Operation operation = Operation::Uninstall;
    PackageSource source = PackageSource::Apt;
    QString packageId;
    std::optional<InstallScope> installScope;
    QString displayName;
    QString currentVersion;
    QString targetVersion;
    bool requiresAuth = false;
    AuthMethod authMethod = AuthMethod::None;
    bool isProtected = false;
    std::optional<QString> protectionReason;
    QVector<PlanStep> steps;
    qint64 createdAtMs = 0;

    [[nodiscard]] QVariantMap toVariantMap() const;
};

} // namespace scope
