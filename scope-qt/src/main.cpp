#include "app/OperationController.h"
#include "app/PackagesController.h"
#include "icons/IconProvider.h"
#include "qml/Theme.h"

#include <QGuiApplication>
#include <QImage>
#include <QQmlApplicationEngine>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QTimer>
#include <QQmlContext>

int main(int argc, char* argv[])
{
    QGuiApplication app(argc, argv);
    app.setOrganizationName(QStringLiteral("Scope"));
    app.setApplicationName(QStringLiteral("Scope"));
    app.setApplicationVersion(QStringLiteral("0.1.0"));

    QQuickStyle::setStyle(QStringLiteral("Basic"));

    scope::PackagesController packages;
    scope::OperationController operations(&packages);
    scope::ThemeColors themeColors;

    QQmlApplicationEngine engine;
    engine.addImageProvider(QStringLiteral("scopeicon"), new scope::IconProvider());

    engine.rootContext()->setContextProperty(QStringLiteral("Theme"), &themeColors);
    engine.rootContext()->setContextProperty(QStringLiteral("Packages"), &packages);
    engine.rootContext()->setContextProperty(QStringLiteral("Operations"), &operations);

    const QUrl url(QStringLiteral("qrc:/qt/qml/scope/qml/Main.qml"));
    QObject::connect(
        &engine, &QQmlApplicationEngine::objectCreationFailed, &app,
        []() { QCoreApplication::exit(-1); }, Qt::QueuedConnection);
    engine.loadFromModule(QStringLiteral("scope"), QStringLiteral("Main"));

    // Headless helper: --screenshot <file> renders the UI and exits.
    const QStringList args = app.arguments();
    const int shotIdx = args.indexOf(QStringLiteral("--screenshot"));
    if (shotIdx >= 0 && shotIdx + 1 < args.size() && !engine.rootObjects().isEmpty()) {
        const QString out = args.at(shotIdx + 1);
        auto* window = qobject_cast<QQuickWindow*>(engine.rootObjects().first());
        if (window) {
            QTimer::singleShot(10000, window, [window, out]() {
                QImage image = window->grabWindow();
                image.save(out);
                QCoreApplication::exit(0);
            });
            return app.exec();
        }
    }

    return app.exec();
}
