#include "OperationTypes.h"

namespace scope {

QVariantMap OperationPlan::toVariantMap() const
{
    QVariantMap map;
    map.insert(QStringLiteral("plan_id"), planId);
    map.insert(QStringLiteral("operation"), operationId(operation));
    map.insert(QStringLiteral("source"), sourceId(source));
    map.insert(QStringLiteral("package_id"), packageId);
    if (installScope)
        map.insert(QStringLiteral("install_scope"), scopeId(*installScope));
    map.insert(QStringLiteral("display_name"), displayName);
    map.insert(QStringLiteral("current_version"), currentVersion);
    map.insert(QStringLiteral("target_version"), targetVersion);
    map.insert(QStringLiteral("requires_auth"), requiresAuth);
    map.insert(QStringLiteral("auth_method"),
               authMethod == AuthMethod::Pkexec ? QStringLiteral("pkexec")
                                                : QStringLiteral("none"));
    map.insert(QStringLiteral("protected"), isProtected);
    if (protectionReason)
        map.insert(QStringLiteral("protection_reason"), *protectionReason);

    QVariantList stepList;
    stepList.reserve(steps.size());
    for (const PlanStep& step : steps) {
        QVariantMap stepMap;
        stepMap.insert(QStringLiteral("description"), step.description);
        stepMap.insert(QStringLiteral("command_summary"), step.commandSummary);
        stepList.append(stepMap);
    }
    map.insert(QStringLiteral("steps"), stepList);
    map.insert(QStringLiteral("created_at_ms"), QString::number(createdAtMs));
    return map;
}

} // namespace scope
