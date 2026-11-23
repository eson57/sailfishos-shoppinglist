import QtQuick 2.6
import Sailfish.Silica 1.0

Dialog {
    id: dialog

    signal linkAccepted(string text)

    allowedOrientations: defaultAllowedOrientations
    canAccept: field.text.trim().length > 0
    onAccepted: linkAccepted(field.text.trim())

    Column {
        width: parent.width

        DialogHeader {
            title: Tr.tr("Import list")
            acceptText: Tr.tr("Import")
        }

        TextArea {
            id: field
            width: parent.width
            height: Math.max(implicitHeight, Screen.height / 3)
            label: Tr.tr("Shopping list link")
            placeholderText: Tr.tr("Enter a link from another device")
            wrapMode: Text.WrapAnywhere

            Component.onCompleted: forceActiveFocus()
        }
    }
}
