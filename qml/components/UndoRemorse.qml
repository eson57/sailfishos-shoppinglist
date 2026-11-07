import QtQuick 2.6
import Sailfish.Silica 1.0

// Remorse component which has both an undo and commit action.
// Commit fires when the remorse times out (as with the standard Remorse)
// The undo action fires if the remorse is clicked
Item {
    id: root

    property var commitAction
    property var undoAction

    function show(message, commit, undo) {
        if (remorse.active)
            remorse.trigger()

        commitAction = commit
        undoAction = undo

        remorse.execute(message, function() {
            var action = root.commitAction

            root.commitAction = null
            root.undoAction = null

            if (action)
                action()
        })
    }

    RemorsePopup {
        id: remorse

        onCanceled: {
            var action = root.undoAction

            root.commitAction = null
            root.undoAction = null

            if (action)
                action()
        }
    }
}
