'use strict';

const KeyList = {
    dom: null,
    renameState: null,

    init: function (dom) {
        this.dom = dom;
    },

    render: function () {
        this.dom.keyList.innerHTML = State.data.keys.map(function (key) {
            return '<div class="key-item" tabindex="-1" data-key="' + Utils.escapeHtml(key) + '" onclick="KeyList.requestSelect(\'' + Utils.escapeJs(key) + '\')">' +
                '<span class="key-name">' + Utils.escapeHtml(key) + '</span>' +
                '<div class="key-actions">' +
                '<button class="key-action-btn" onclick="event.stopPropagation(); KeyList.rename(\'' + Utils.escapeJs(key) + '\')" title="Rename">&#9998;</button>' +
                '<button class="key-action-btn" onclick="event.stopPropagation(); KeyList.trash(\'' + Utils.escapeJs(key) + '\')" title="Delete">&#128465;</button>' +
                '</div>' +
                '</div>';
        }).join('');
        this.updateSelection();
    },

    renderTrash: function () {
        if (State.data.trashedKeys.length === 0) {
            this.dom.trashSection.style.display = 'none';
            return;
        }
        this.dom.trashSection.style.display = 'block';
        this.dom.trashCount.textContent = State.data.trashedKeys.length.toString();
        this.dom.trashList.innerHTML = State.data.trashedKeys.map(function (key) {
            return '<div class="trash-item" tabindex="-1" data-key="' + Utils.escapeHtml(key) + '" onclick="KeyList.requestSelect(\'' + Utils.escapeJs(key) + '\')">' +
                '<span class="trash-icon">T</span>' +
                '<span class="key-name">' + Utils.escapeHtml(key) + '</span>' +
                '</div>';
        }).join('');
        this.updateSelection();
    },

    updateSelection: function () {
        const selectedKey = State.data.selectedKey;
        document.querySelectorAll('.key-item').forEach(function (el) {
            el.classList.toggle('selected', el.dataset.key === selectedKey);
        });
        document.querySelectorAll('.trash-item').forEach(function (el) {
            el.classList.toggle('selected', el.dataset.key === selectedKey);
        });
    },

    requestSelect: function (keyName) {
        if (keyName === State.data.selectedKey) {
            return;
        }

        const isTrashed = State.data.trashedKeys.indexOf(keyName) !== -1;
        State.setSelectedKey(keyName, isTrashed);
        this.updateSelection();
        Main.updateSearchButton();
        Editor.showEmpty('Loading...');
        Api.send({type: 'select', key: keyName});
    },

    rename: function (keyName) {
        // Cancel any existing rename
        this.cancelRename();

        const item = document.querySelector('.key-item[data-key="' + CSS.escape(keyName) + '"]');
        if (!item) return;

        this.renameState = {
            oldKey: keyName,
            element: item
        };

        item.classList.add('editing');
        item.style.position = 'relative';

        const input = document.createElement('input');
        input.type = 'text';
        input.className = 'rename-input';
        input.value = keyName;
        item.appendChild(input);

        input.focus();
        input.select();

        const self = this;
        input.addEventListener('click', function (e) {
            e.stopPropagation();
        });
        input.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                e.preventDefault();
                input.blur();
            } else if (e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                self.cancelRename();
            }
        });

        input.addEventListener('blur', function () {
            // Small delay to allow click events to fire first
            setTimeout(function () {
                if (self.renameState && self.renameState.element === item) {
                    self.submitRename();
                }
            }, 100);
        });
    },

    submitRename: function () {
        if (!this.renameState) return;

        const input = this.renameState.element.querySelector('.rename-input');
        if (!input) return;

        const newName = input.value.trim();
        const oldName = this.renameState.oldKey;

        // Clear any existing error
        this.clearRenameError();

        // Validate
        if (!newName) {
            this.showRenameError('Key name cannot be empty');
            input.focus();
            return;
        }
        if (newName.length > 256) {
            this.showRenameError('Key name cannot exceed 256 characters');
            input.focus();
            return;
        }
        if (newName === oldName) {
            this.cancelRename();
            return;
        }

        // Send rename request (force: false, will get confirmation if exists)
        Api.send({type: 'rename', oldKey: oldName, newKey: newName, force: false});
    },

    cancelRename: function () {
        if (!this.renameState) return;

        const item = this.renameState.element;
        item.classList.remove('editing');
        item.style.position = '';

        const input = item.querySelector('.rename-input');
        if (input) input.remove();

        const error = item.querySelector('.rename-error');
        if (error) error.remove();

        this.renameState = null;
    },

    showRenameError: function (message) {
        if (!this.renameState) return;

        const item = this.renameState.element;
        const input = item.querySelector('.rename-input');
        if (input) input.classList.add('invalid');

        // Remove existing error
        const existing = item.querySelector('.rename-error');
        if (existing) existing.remove();

        const error = document.createElement('div');
        error.className = 'rename-error';
        error.textContent = message;
        item.appendChild(error);
    },

    clearRenameError: function () {
        if (!this.renameState) return;

        const item = this.renameState.element;
        const input = item.querySelector('.rename-input');
        if (input) input.classList.remove('invalid');

        const error = item.querySelector('.rename-error');
        if (error) error.remove();
    },

    handleRenameResult: function (oldKey, newKey, result) {
        if (result === 'success') {
            // Remove destination key if it existed (overwrite case)
            const destIdx = State.data.keys.indexOf(newKey);
            if (destIdx !== -1) {
                State.data.keys.splice(destIdx, 1);
            }
            // Update source key in place (maintain position, don't re-search)
            const srcIdx = State.data.keys.indexOf(oldKey);
            if (srcIdx !== -1) {
                State.data.keys[srcIdx] = newKey;
            }
            // Update selection if it was the source or destination key
            if (State.data.selectedKey === oldKey || State.data.selectedKey === newKey) {
                const wasDestination = State.data.selectedKey === newKey;
                State.setSelectedKey(newKey, false);
                if (wasDestination) {
                    // Destination was selected - content changed, need to reload
                    Api.send({type: 'select', key: newKey});
                }
            }
            this.cancelRename();
            this.render();
        } else if (result === 'destinationExists') {
            // Show confirmation dialog
            this.showOverwriteDialog(oldKey, newKey);
        } else if (result === 'invalidKey') {
            this.showRenameError('Invalid key name');
        } else if (result === 'notFound') {
            this.showRenameError('Key not found');
            this.cancelRename();
        }
    },

    showOverwriteDialog: function (oldKey, newKey) {
        const self = this;

        Dialog.show({
            message: 'Key "' + newKey + '" already exists. Overwrite?',
            buttons: [
                { label: 'Cancel', action: 'cancel' },
                { label: 'Overwrite', action: 'overwrite', primary: true }
            ],
            onClose: function (action) {
                self.cancelRename();
                if (action === 'overwrite') {
                    Api.send({type: 'rename', oldKey: oldKey, newKey: newKey, force: true});
                }
            }
        });
    },

    trash: function (keyName) {
        Api.send({type: 'trash', key: keyName});
    }
};
