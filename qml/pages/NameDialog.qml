import QtQuick 2.6
import Sailfish.Silica 1.0

Dialog {
    id: root

    property string titleText
    property string initialText
    signal nameAccepted(string name)

    allowedOrientations: defaultAllowedOrientations
    canAccept: field.text.trim().length > 0
    onAccepted: nameAccepted(field.text.trim())

    Column {
        width: parent.width

        DialogHeader {
            title: root.titleText
        }

        TextField {
            id: field
            width: parent.width
            placeholderText: root.titleText
            EnterKey.enabled: root.canAccept
            EnterKey.iconSource: "image://theme/icon-m-enter-accept"
            EnterKey.onClicked: root.accept()

            Component.onCompleted: {
                text = root.initialText
                forceActiveFocus()
            }
        }
    }
}
