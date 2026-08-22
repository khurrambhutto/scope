#include "core/desktop/DesktopEntry.h"
#include "core/icons/IconResolver.h"
#include "core/scanner/AppImageScanner.h"
#include "core/scanner/FlatpakScanner.h"

#include <QRegularExpression>
#include <QTemporaryDir>
#include <QtTest>

using namespace scope;

class TestParsers : public QObject
{
    Q_OBJECT

private slots:
    void execTokenStripsEnvAndFieldCodes()
    {
        QCOMPARE(execBinaryToken("myapp %u"), QString("myapp"));
        QCOMPARE(execBinaryToken("env LD_LIBRARY_PATH=/opt/lib /opt/MyApp/bin/run %F"),
                 QString("/opt/MyApp/bin/run"));
        QCOMPARE(execBinaryToken("\"/opt/My App/myapp\" --flag"),
                 QString("/opt/My App/myapp"));
        QCOMPARE(execBinaryToken("sh -c \"exec app\""), QString("sh"));
        QVERIFY(!execBinaryToken("%u %f").has_value());
    }

    void desktopFileParsing()
    {
        QTemporaryDir dir;
        const QString path = dir.filePath("Test App.desktop");
        {
            QFile f(path);
            QVERIFY(f.open(QIODevice::WriteOnly | QIODevice::Text));
            f.write(R"([Desktop Entry]
Type=Application
Name=Test App
Name[de]=Testanwendung
Comment=A test application
Exec=/opt/test-app/bin/tapp %u
Icon=test-app-icon
Terminal=false
Categories=Development;IDE;
NoDisplay=false
)");
        }

        const auto app = parseDesktopFile(path, QStringLiteral("Test App"));
        QVERIFY(app.has_value());
        QCOMPARE(app->name, QStringLiteral("Test App")); // bare key wins over locale
        QCOMPARE(app->exec, QStringLiteral("/opt/test-app/bin/tapp %u"));
        QCOMPARE(app->comment, QStringLiteral("A test application"));
        QCOMPARE(app->icon, QStringLiteral("test-app-icon"));
        QCOMPARE(app->categories.size(), 2);
        QVERIFY(!app->terminal);
        QVERIFY(!app->noDisplay);
    }

    void desktopFileRejectsNonApps()
    {
        QTemporaryDir dir;
        {
            QFile f(dir.filePath("link.desktop"));
            QVERIFY(f.open(QIODevice::WriteOnly));
            f.write("[Desktop Entry]\nType=Link\nURL=http://example.com\n");
        }
        {
            QFile f(dir.filePath("hidden.desktop"));
            QVERIFY(f.open(QIODevice::WriteOnly));
            f.write("[Desktop Entry]\nType=Application\nName=H\nExec=x\nNoDisplay=true\n");
        }
        {
            QFile f(dir.filePath("noexec.desktop"));
            QVERIFY(f.open(QIODevice::WriteOnly));
            f.write("[Desktop Entry]\nType=Application\nName=N\n");
        }
        QVERIFY(!parseDesktopFile(dir.filePath("link.desktop"), "link").has_value());
        QVERIFY(!parseDesktopFile(dir.filePath("hidden.desktop"), "hidden").has_value());
        QVERIFY(!parseDesktopFile(dir.filePath("noexec.desktop"), "noexec").has_value());
    }

    void flatpakSizeParsing()
    {
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("1,5 GB")), quint64(1610612736));
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("512 kB")), quint64(524288));
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("2 MB")), quint64(2097152));
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("64.0 B")), quint64(64));
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("1 TB")),
                 quint64(1099511627776LL));
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("")), quint64(0));
        QCOMPARE(flatpak_detail::parseSize(QStringLiteral("garbage")), quint64(0));
    }

    void appImageNameAndVersionHeuristics()
    {
        QRegularExpression tail(
            QStringLiteral("[-_]?(v?\\d[\\d.]*|x86_64|amd64|aarch64|arm64|linux).*$"),
            QRegularExpression::CaseInsensitiveOption);
        QString stem = QStringLiteral("MyTool-1.2.3-x86_64");
        stem.remove(tail);
        QCOMPARE(stem, QString("MyTool"));

        const QRegularExpression version(QStringLiteral("[_-]v?(\\d+(?:\\.\\d+){1,3})"));
        const auto m = version.match(QStringLiteral("MyTool-1.2.3-x86_64"));
        QVERIFY(m.hasMatch());
        QCOMPARE(m.captured(1), QString("1.2.3"));
    }

    void percentEncodingRoundTrip()
    {
        const QString raw = QDir::homePath() + "/AppImages/My App (v2)/icon_ü.png";
        const QString encoded = iconurl::encode(raw);
        QVERIFY(!encoded.contains(QLatin1Char(' ')));
        QVERIFY(!encoded.contains(QLatin1Char('(')));
        QCOMPARE(iconurl::decode(encoded), raw);
    }
};

QTEST_MAIN(TestParsers)
#include "test_parsers.moc"
