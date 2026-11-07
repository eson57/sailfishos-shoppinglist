#include "sailors/src/sailfishapp.h"

#include <cstring>

#include <QtCore/QDebug>
#include <QtCore/QLocale>
#include <QtCore/QTranslator>
#include <QtQml/QQmlContext>

#include <sailfishapp.h>

namespace sailors {

QmlApplicationHolder::QmlApplicationHolder(int& argc, char** argv)
  : app(SailfishApp::application(argc, argv))
  , view(SailfishApp::createView())
{
  QObject::connect(view->engine(),
                   &QQmlEngine::quit,
                   view.get(),
                   &QWindow::close,
                   Qt::QueuedConnection);
}

std::unique_ptr<QmlApplicationHolder>
newQmlApplicationHolder(const rust::Vec<rust::String>& args)
{
  // QGuiApplication keeps references to argc and argv for its whole lifetime.
  static int argc = 0;
  static char** argv = nullptr;
  if (argv == nullptr) {
    argc = static_cast<int>(args.size());
    argv = new char*[argc + 1];
    argv[argc] = nullptr;
    for (int i = 0; i < argc; ++i) {
      const auto& arg = args[i];
      argv[i] = new char[arg.size() + 1];
      std::memcpy(argv[i], arg.data(), arg.size());
      argv[i][arg.size()] = '\0';
    }
  }
  return std::make_unique<QmlApplicationHolder>(argc, argv);
}

void
setContextProperty(QmlApplicationHolder& self,
                   const QString& name,
                   const QVariant& value)
{
  self.view->engine()->rootContext()->setContextProperty(name, value);
}

void
setContextObject(QmlApplicationHolder& self, const QString& name, QObject* obj)
{
  self.view->engine()->rootContext()->setContextProperty(name, obj);
}

QQmlEngine&
engine(QmlApplicationHolder& self)
{
  return *self.view->engine();
}

void
promoteGuiAppToQmlContext(QmlApplicationHolder& self, const QString& name)
{
  self.view->engine()->rootContext()->setContextProperty(name, self.app.get());
}

QUrl
pathTo(const QString& path)
{
  return SailfishApp::pathTo(path);
}

void
addImportPath(QmlApplicationHolder& self, const QString& path)
{
  self.view->engine()->addImportPath(path);
}

::std::int32_t
exec(QmlApplicationHolder& self)
{
  return self.app->exec();
}

void
setSource(QmlApplicationHolder& self, const QUrl& src)
{
  self.view->setSource(src);
}

void
setTitle(QmlApplicationHolder& self, const QString& title)
{
  self.view->setTitle(title);
}

void
setApplicationVersion(QmlApplicationHolder& self, const QString& version)
{
  self.app->setApplicationVersion(version);
}

void
setQuitOnLastWindowClosed(QmlApplicationHolder& self, bool quit)
{
  self.app->setQuitOnLastWindowClosed(quit);
}

::std::int32_t
installDefaultTranslator(QmlApplicationHolder& self)
{
  const QString transDir =
    SailfishApp::pathTo(QStringLiteral("translations")).toLocalFile();
  const QString appName = qApp->applicationName();
  QTranslator translator(qApp);
  int result = 0;
  if (!translator.load(QLocale(), appName, "-", transDir)) {
    qWarning() << "Failed to load translator for"
               << QLocale::system().uiLanguages() << "Searched" << transDir
               << "for" << appName;
    result = 1;
    if (!translator.load(appName, transDir)) {
      qWarning() << "Could not load default translator either!";
      result = 2;
    }
  }
  self.app->installTranslator(&translator);
  return result;
}

void
show(QmlApplicationHolder& self)
{
  self.view->show();
}

void
showFullScreen(QmlApplicationHolder& self)
{
  self.view->showFullScreen();
}

} // namespace sailors
