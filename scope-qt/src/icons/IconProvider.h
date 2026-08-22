#pragma once

#include <QQuickImageProvider>
#include <QHash>

namespace scope {

// Serves icon files over "image://scopeicon/<encoded-path>". Only paths that
// the backend itself resolved and registered may be loaded — the QML layer has
// no general filesystem access (same invariant as the Tauri custom scheme).
class IconProvider : public QQuickImageProvider
{
public:
    explicit IconProvider();

    QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override;

private:
    QHash<QString, QImage> m_cache;
    static constexpr int kMaxCached = 512;
};

} // namespace scope
