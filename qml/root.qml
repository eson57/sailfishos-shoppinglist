import QtQuick 2.6
import Sailfish.Silica 1.0
import Nemo.DBus 2.0
import ShoppingList 1.0

import "pages"

ApplicationWindow {
    id: window

    allowedOrientations: defaultAllowedOrientations
    initialPage: Component {
        HomePage {
            shoppingLists: shoppingListsModel
        }
    }

    cover: Component {
        CoverPage {
            shoppingLists: shoppingListsModel
        }
    }

    ShoppingListsModel {
        id: shoppingListsModel
        registry: ActorRegistry
        onOpen: window.openShoppingList(shopping_list_id)
        onReadyChanged: window.importLaunchLink()
    }

    // This is a bit funky. If the app is launched with a share url, we store
    // the url in the Qt UI to then invoke the same `importLink` method that the
    // dbus interface uses.
    property string launchLink: LaunchUrl

    function importLaunchLink() {
        if (!shoppingListsModel.ready || launchLink.length === 0)
            return
        var link = launchLink
        launchLink = ""
        shoppingListsModel.importLink(link)
    }

    function openShoppingList(shoppingListId) {
        pageStack.push(Qt.resolvedUrl("pages/ShoppingListPage.qml"), {
            "shoppingListId": shoppingListId,
        })
    }

    DBusAdaptor {
        service: DBusName
        path: DBusPath
        iface: DBusName.replace(/-/g, "_")
        bus: DBus.SessionBus

        function openLink(url) {
            console.log("openLink called with", url)
            window.activate()
            if (url && url.length > 0)
                shoppingListsModel.importLink(url)
        }
    }
}
