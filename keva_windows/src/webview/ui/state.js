'use strict';

const State = {
    data: {
        keys: [],
        trashedKeys: [],
        exactMatch: 'none',
        selectedKey: null,
        isSelectedTrashed: false,
        isDirty: false,
        isCopying: false,
        isShuttingDown: false,
        pendingSelectKey: null,
        pendingLinkInsert: null,
        focusEditorOnLoad: false,
        currentTheme: 'dark',
        activePane: 'search',
        attachments: [],
        editorMode: 'edit',
    },

    setSelectedKey: function (key, isTrashed) {
        if (this.data.isDirty && this.data.selectedKey && this.data.selectedKey !== key) {
            Editor.forceSave();
        }
        this.data.selectedKey = key;
        this.data.isSelectedTrashed = isTrashed;
    },

    clearSelection: function () {
        if (this.data.isDirty && this.data.selectedKey) {
            Editor.forceSave();
        }
        this.data.selectedKey = null;
        this.data.isSelectedTrashed = false;
    },

    isKeyVisible: function (key) {
        return this.data.keys.indexOf(key) !== -1
            || this.data.trashedKeys.indexOf(key) !== -1;
    }
};
