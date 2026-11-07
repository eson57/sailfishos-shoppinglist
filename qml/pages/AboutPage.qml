import QtQuick 2.6
import Sailfish.Silica 1.0
import Opal.About 1.0 as A

import "../generated"

A.AboutPageBase {
    id: root
    allowedOrientations: Orientation.All
    appName: "Shopping List"
    appVersion: Qt.application.version
    appIcon: Qt.resolvedUrl("../images/%1.png".arg(Qt.application.name))
    description: Tr.tr("A shopping list program with p2p sharing")
    authors: ["2026-%1 Ben Simms".arg((new Date()).getFullYear())]
    licenses: A.License { spdxId: "GPL-3.0-or-later" }
    sourcesUrl: "https://github.com/simmsb/sailfishos-shoppinglist"
    attributions: crateAttributions.attributions

    CrateAttributions {
        id: crateAttributions
    }

    PullDownMenu {
        parent: root.flickable

        MenuItem {
            text: Tr.tr("Debug")
            onClicked: pageStack.push(Qt.resolvedUrl("DebugStatusPage.qml"))
        }
    }
}
