import QtQuick 2.6
import Sailfish.Silica 1.0
import Sailfish.Share 1.0
import Amber.Barcode 1.0

Page {
    id: page

    property var shoppingListModel
    property bool copied: false
    allowedOrientations: defaultAllowedOrientations

    Component.onCompleted: {
        if (shoppingListModel)
            shoppingListModel.share()
    }

    ShareAction {
        id: shareHandler
        mimeType: 'text/x-url'
        title: Tr.tr("Share shoppinglist")
    }

    SilicaFlickable {
        anchors.fill: parent
        contentHeight: Math.max(column.height, height)

        Column {
            id: column

            spacing: Theme.paddingLarge

            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
                leftMargin: Theme.horizontalPageMargin
                rightMargin: Theme.horizontalPageMargin
            }

            PageHeader {
                title: Tr.tr("Share")
            }

            Label {
                width: parent.width

                text: Tr.tr("Share this list with someone, they'll be able to view and edit it.")
                wrapMode: Text.Wrap
                color: Theme.secondaryColor
                font.pixelSize: Theme.fontSizeSmall
            }

            BarcodeImage {
                id: code

                anchors.horizontalCenter: parent.horizontalCenter
                width: parent.width
                height: width

                format: BarcodeImage.QRCode
                sourceText: shoppingListModel.shareLink
                visible: sourceText.length > 0 && error.length === 0
            }

            Label {
                visible: code.error.length > 0
                text: code.error

                color: Theme.highlightColor

                wrapMode: Text.Wrap
                font.pixelSize: Theme.fontSizeSmall
            }

            Label {
                width: parent.width

                text: shoppingListModel.shareLink
                wrapMode: Text.WrapAnywhere

                color: Theme.primaryColor
                font.pixelSize: Theme.fontSizeExtraSmall
                textFormat: Text.PlainText
            }

            ButtonLayout {
                Button {
                    text: Tr.tr("Share")
                    onClicked: {
                        shareHandler.resources = [{
                            'type': 'text/x-url',
                            'status': shoppingListModel.shareLink
                        }]
                        shareHandler.trigger()
                    }
                }

                Button {
                    text: page.copied ? Tr.tr("Copied") : Tr.tr("Copy link")
                    onClicked: {
                        Clipboard.text = shoppingListModel.shareLink
                        page.copied = true
                        copyTimer.restart()
                    }
                }
            }

        }

        VerticalScrollDecorator {}
    }

    Timer {
        id: copyTimer
        interval: 2000
        onTriggered: page.copied = false
    }
}
