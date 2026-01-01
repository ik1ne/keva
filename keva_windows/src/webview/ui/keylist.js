'use strict';

const KeyList = {
    dom: null,

    init: function (dom) {
        this.dom = dom;
    },

    render: function () {
        const self = this;
        this.dom.keyList.innerHTML = State.data.keys.map(function (key) {
            return '<div class="key-item" data-key="' + self.escapeHtml(key.name) + '" onclick="KeyList.requestSelect(\'' + self.escapeJs(key.name) + '\')">' +
                '<span class="key-name">' + self.escapeHtml(key.name) + '</span>' +
                '<div class="key-actions">' +
                '<button class="key-action-btn" onclick="event.stopPropagation(); KeyList.rename(\'' + self.escapeJs(key.name) + '\')" title="Rename">&#9998;</button>' +
                '<button class="key-action-btn" onclick="event.stopPropagation(); KeyList.trash(\'' + self.escapeJs(key.name) + '\')" title="Delete">&#128465;</button>' +
                '</div>' +
                '</div>';
        }).join('');
        this.updateSelection();
    },

    renderTrash: function () {
        const self = this;
        if (State.data.trashedKeys.length === 0) {
            this.dom.trashSection.style.display = 'none';
            return;
        }
        this.dom.trashSection.style.display = 'block';
        this.dom.trashCount.textContent = State.data.trashedKeys.length.toString();
        this.dom.trashList.innerHTML = State.data.trashedKeys.map(function (key) {
            return '<div class="trash-item" onclick="KeyList.requestSelect(\'' + self.escapeJs(key.name) + '\')">' +
                '<span class="trash-icon">T</span>' +
                '<span class="key-name">' + self.escapeHtml(key.name) + '</span>' +
                '</div>';
        }).join('');
    },

    updateSelection: function () {
        document.querySelectorAll('.key-item').forEach(function (el) {
            el.classList.toggle('selected', el.dataset.key === State.data.selectedKey);
        });
    },

    requestSelect: function (keyName) {
        if (State.data.isDirty && State.data.selectedKey) {
            Editor.forceSave();
        }
        const isTrashed = State.data.trashedKeys.some(function (k) {
            return k.name === keyName;
        });
        State.setSelectedKey(keyName, isTrashed);
        this.updateSelection();
        Main.updateSearchButton();
        Editor.showEmpty('Loading...');
        Api.send({type: 'select', key: keyName});
    },

    rename: function (keyName) {
        const newName = prompt('Rename key:', keyName);
        if (newName && newName !== keyName) {
            Api.send({type: 'rename', oldKey: keyName, newKey: newName});
        }
    },

    trash: function (keyName) {
        Api.send({type: 'trash', key: keyName});
    },

    escapeHtml: function (str) {
        return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    },

    escapeJs: function (str) {
        return str.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
    }
};
