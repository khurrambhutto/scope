#include "IconProvider.h"

#include "icons/IconResolver.h"

#include <QFile>
#include <QFileInfo>
#include <QImageReader>

namespace scope {

IconProvider::IconProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
{
}

QImage IconProvider::requestImage(const QString& id, QSize* size, const QSize& requestedSize)
{
    const QString path = iconurl::decode(id);

    if (!IconResolver::isRegisteredPath(path))
        return {};

    const auto cached = m_cache.constFind(path);
    if (cached != m_cache.cend()) {
        if (size)
            *size = cached->size();
        return *cached;
    }

    QImageReader reader(path);
    QImage image = reader.read();
    if (image.isNull())
        return {};

    if (requestedSize.isValid() && requestedSize.width() > 0 && requestedSize.height() > 0 &&
        (requestedSize.width() < image.width() || requestedSize.height() < image.height())) {
        image = image.scaled(requestedSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    }

    if (m_cache.size() >= kMaxCached)
        m_cache.clear();
    m_cache.insert(path, image);

    if (size)
        *size = image.size();
    return image;
}

} // namespace scope
