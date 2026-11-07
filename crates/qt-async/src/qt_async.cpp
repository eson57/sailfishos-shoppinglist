#include "qt-async/src/qt_async.h"

#include <atomic>

#include <QtCore/QCoreApplication>
#include <QtCore/QEvent>
#include <QtCore/QObject>

#include "qt-async/src/lib.cxx.h"

namespace {

class LocalSetPoller : public QObject
{
public:
  bool event(QEvent* event) override
  {
    if (event->type() == QEvent::User) {
      qt_async::poll_local_set();
      return true;
    }
    return QObject::event(event);
  }
};

std::atomic<LocalSetPoller*> poller{ nullptr };

} // namespace

namespace qt_async {

void
installPoller()
{
  poller.store(new LocalSetPoller());
}

void
schedulePoll()
{
  if (auto* target = poller.load()) {
    QCoreApplication::postEvent(target, new QEvent(QEvent::User));
  }
}

} // namespace qt_async
