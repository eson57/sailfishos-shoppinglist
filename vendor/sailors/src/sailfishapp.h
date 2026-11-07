#pragma once

#include <memory>

#include <QtCore/QObject>
#include <QtCore/QString>
#include <QtCore/QUrl>
#include <QtCore/QVariant>
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlEngine>
#include <QtQuick/QQuickView>

#include "rust/cxx.h"

namespace sailors {

using QQmlEngine = ::QQmlEngine;

struct QmlApplicationHolder
{
  std::unique_ptr<QGuiApplication> app;
  std::unique_ptr<QQuickView> view;

  QmlApplicationHolder(int& argc, char** argv);
};

std::unique_ptr<QmlApplicationHolder>
newQmlApplicationHolder(const rust::Vec<rust::String>& args);

void
setContextProperty(QmlApplicationHolder& self,
                   const QString& name,
                   const QVariant& value);
void
setContextObject(QmlApplicationHolder& self, const QString& name, QObject* obj);
QQmlEngine&
engine(QmlApplicationHolder& self);
void
promoteGuiAppToQmlContext(QmlApplicationHolder& self, const QString& name);
QUrl
pathTo(const QString& path);
void
addImportPath(QmlApplicationHolder& self, const QString& path);
::std::int32_t
exec(QmlApplicationHolder& self);
void
setSource(QmlApplicationHolder& self, const QUrl& src);
void
setTitle(QmlApplicationHolder& self, const QString& title);
void
setApplicationVersion(QmlApplicationHolder& self, const QString& version);
void
setQuitOnLastWindowClosed(QmlApplicationHolder& self, bool quit);
::std::int32_t
installDefaultTranslator(QmlApplicationHolder& self);
void
show(QmlApplicationHolder& self);
void
showFullScreen(QmlApplicationHolder& self);

} // namespace sailors
