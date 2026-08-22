#include "operations/OperationTypes.h"
#include "operations/PlanStore.h"
#include "package/Package.h"

#include <QThread>
#include <QtTest>

using namespace scope;

class TestPlanStore : public QObject
{
    Q_OBJECT

private slots:
    void takeIsOneShot()
    {
        PlanStore store;
        OperationPlan plan;
        plan.planId = newPlanId(store.nextCounter());
        plan.operation = Operation::Uninstall;
        plan.source = PackageSource::Snap;
        plan.packageId = QStringLiteral("firefox");
        plan.displayName = QStringLiteral("Firefox");

        store.issue(plan);

        auto taken = store.take(plan.planId);
        QVERIFY(taken.has_value());
        QCOMPARE(taken->packageId, QStringLiteral("firefox"));

        // Second take must fail: plans are single-use.
        QVERIFY(!store.take(plan.planId).has_value());
    }

    void unknownPlanRejected()
    {
        PlanStore store;
        QVERIFY(!store.take(QStringLiteral("plan-1-1")).has_value());
    }

    void expiredPlansAreStale()
    {
        // 50ms TTL so the test stays fast.
        PlanStore store(50);
        OperationPlan plan;
        plan.planId = newPlanId(store.nextCounter());
        store.issue(plan);

        QThread::msleep(80);
        QVERIFY(!store.take(plan.planId).has_value());
    }

    void issuePrunesExpiredEntries()
    {
        PlanStore store(50);
        for (int i = 0; i < 5; ++i) {
            OperationPlan plan;
            plan.planId = newPlanId(store.nextCounter());
            store.issue(plan);
        }
        QThread::msleep(80);

        // Issuing again must not resurrect expired entries.
        OperationPlan fresh;
        fresh.planId = newPlanId(store.nextCounter());
        store.issue(fresh);

        QVERIFY(store.take(fresh.planId).has_value());
    }
};

QTEST_MAIN(TestPlanStore)
#include "test_planstore.moc"
