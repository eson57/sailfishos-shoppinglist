#include "sailfishos-shoppinglist/src/cpp/app.h"

#include <QtCore/QCoreApplication>
#include <QtQml/QtQml>

#include "sailfishos-shoppinglist/src/actor/registry.cxxqt.h"
#include "sailfishos-shoppinglist/src/gui/debug_status.cxxqt.h"
#include "sailfishos-shoppinglist/src/gui/home.cxxqt.h"
#include "sailfishos-shoppinglist/src/gui/shopping_list.cxxqt.h"

namespace shoppinglist {

void
connectAboutToQuit(ActorRegistry& registry)
{
  QObject::connect(QCoreApplication::instance(),
                   &QCoreApplication::aboutToQuit,
                   &registry,
                   [&registry]() { registry.finishShutdown(); },
                   Qt::DirectConnection);
}

void
registerQmlTypes()
{
  qmlRegisterType<ShoppingListsModel>(
    "ShoppingList", 1, 0, "ShoppingListsModel");
  qmlRegisterType<ShoppingListModel>("ShoppingList", 1, 0, "ShoppingListModel");
  qmlRegisterType<DebugStatusModel>("ShoppingList", 1, 0, "DebugStatusModel");

  qmlRegisterType<ActorRegistry>();
  qmlRegisterType<HomeListModel>();
  qmlRegisterType<ItemListModel>();
  qmlRegisterType<StatusRowsModel>();
  qmlRegisterType<LogEntriesModel>();
}

} // namespace shoppinglist
