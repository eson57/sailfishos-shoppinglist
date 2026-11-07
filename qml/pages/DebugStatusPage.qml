import QtQuick 2.6
import Sailfish.Silica 1.0
import ShoppingList 1.0

Page {
    id: page

    allowedOrientations: defaultAllowedOrientations

    DebugStatusModel {
        id: debugStatus
        registry: ActorRegistry
    }

    Timer {
        interval: 2000
        repeat: true
        running: page.status === PageStatus.Active && debugStatus.ready
        onTriggered: debugStatus.refresh()
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: column.height

        VerticalScrollDecorator {}

        Column {
            id: column
            width: parent.width

            PageHeader {
                title: Tr.tr("Debug")
            }

            Repeater {
                model: debugStatus.statusRows

                delegate: Item {
                    width: parent.width
                    height: model.isHeader ? heading.height : detail.height

                    SectionHeader {
                        id: heading
                        visible: model.isHeader
                        text: model.label
                    }

                    DetailItem {
                        id: detail
                        visible: !model.isHeader
                        label: model.label
                        value: model.value
                    }
                }
            }

            SectionHeader {
                text: Tr.tr("Log")
            }

            SilicaListView {
                id: logView
                width: parent.width
                height: Math.max(Theme.itemSizeHuge * 3, page.height * 0.6)
                clip: true
                model: debugStatus.logEntries

                delegate: Item {
                    width: logView.width
                    height: entry.height

                    Label {
                        id: entry
                        anchors {
                            left: parent.left
                            right: parent.right
                            leftMargin: Theme.horizontalPageMargin
                            rightMargin: Theme.horizontalPageMargin
                        }
                        wrapMode: Text.Wrap
                        font.pixelSize: Theme.fontSizeExtraSmall
                        color: model.level === "ERROR" ? "#e05050"
                             : model.level === "WARN" ? "#e0a030"
                             : Theme.secondaryColor
                        text: model.text
                    }
                }

                VerticalScrollDecorator {}
            }
        }
    }
}
