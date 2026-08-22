#include "PlanStore.h"

#include <QDateTime>

namespace scope {

QString newPlanId(int counter)
{
    return QStringLiteral("plan-%1-%2")
        .arg(QDateTime::currentMSecsSinceEpoch())
        .arg(counter);
}

PlanStore::PlanStore(qint64 ttlMs)
    : m_ttlMs(ttlMs)
{
}

void PlanStore::pruneLocked(qint64 nowMs)
{
    const qint64 cutoff = nowMs - m_ttlMs;
    for (auto it = m_plans.begin(); it != m_plans.end();) {
        if (it.value().createdAtMs <= cutoff)
            it = m_plans.erase(it);
        else
            ++it;
    }
}

void PlanStore::issue(const OperationPlan& plan)
{
    const QMutexLocker locker(&m_mutex);
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    pruneLocked(now);
    StoredPlan stored;
    stored.plan = plan;
    stored.createdAtMs = now;
    m_plans.insert(plan.planId, std::move(stored));
}

std::optional<OperationPlan> PlanStore::take(const QString& planId)
{
    const QMutexLocker locker(&m_mutex);
    const qint64 now = QDateTime::currentMSecsSinceEpoch();
    pruneLocked(now);

    const auto it = m_plans.constFind(planId);
    if (it == m_plans.cend())
        return std::nullopt;

    OperationPlan plan = it.value().plan;
    m_plans.erase(it);
    return plan;
}

int PlanStore::nextCounter()
{
    const QMutexLocker locker(&m_mutex);
    return ++m_counter;
}

} // namespace scope
