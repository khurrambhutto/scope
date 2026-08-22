#pragma once

#include "operations/OperationTypes.h"

#include <QHash>
#include <QMutex>

#include <optional>

namespace scope {

// One-shot plan store with a TTL. A plan can be taken exactly once; expired
// plans are rejected. This is the stale-plan guard for apply flows.
class PlanStore
{
public:
    explicit PlanStore(qint64 ttlMs = 5 * 60 * 1000);

    void issue(const OperationPlan& plan);
    [[nodiscard]] std::optional<OperationPlan> take(const QString& planId);

    [[nodiscard]] int nextCounter();

private:
    struct StoredPlan {
        OperationPlan plan;
        qint64 createdAtMs = 0;
    };

    void pruneLocked(qint64 nowMs);

    QMutex m_mutex;
    QHash<QString, StoredPlan> m_plans;
    qint64 m_ttlMs;
    int m_counter = 0;
};

[[nodiscard]] QString newPlanId(int counter);

} // namespace scope
