'use strict';

const Attachments = {
    dom: null,
    listEl: null,
    selectedIndices: new Set(),
    lastClickedIndex: -1,
    renameState: null,

    init: function (dom) {
        this.dom = dom;
        this.listEl = document.getElementById('attachments-list');
        this.setupEventHandlers();
    },

    setupEventHandlers: function () {
        const self = this;
        if (!this.dom.container) return;

        this.dom.container.addEventListener('click', function (e) {
            // Handle action buttons
            const actionBtn = e.target.closest('.attachment-action-btn');
            if (actionBtn) {
                e.stopPropagation();
                const item = actionBtn.closest('.attachment-item');
                const filename = item.dataset.filename;
                const action = actionBtn.dataset.action;

                if (action === 'rename') {
                    self.startRename(filename);
                } else if (action === 'delete') {
                    self.startDelete(filename);
                }
                return;
            }

            const item = e.target.closest('.attachment-item');
            if (!item) return;

            // Don't handle clicks during rename
            if (self.renameState) return;

            const items = self.getItems();
            const index = Array.prototype.indexOf.call(items, item);
            if (index === -1) return;

            Main.setActivePane('attachments');

            if (e.ctrlKey) {
                self.toggleSelect(index);
            } else if (e.shiftKey && self.lastClickedIndex !== -1) {
                self.rangeSelect(self.lastClickedIndex, index);
            } else {
                self.singleSelect(index);
            }

            self.lastClickedIndex = index;
            item.focus();
        });

        this.dom.container.addEventListener('keydown', function (e) {
            // Handle Escape during rename
            if (e.key === 'Escape' && self.renameState) {
                e.preventDefault();
                e.stopPropagation();
                self.cancelRename();
                return;
            }

            if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
                e.preventDefault();
                self.navigateArrow(e.key === 'ArrowDown' ? 1 : -1, e.shiftKey);
            }
        });
    },

    render: function () {
        if (!this.listEl) return;

        const attachments = State.data.attachments;
        this.clearSelection();

        if (attachments.length === 0) {
            this.listEl.innerHTML = '';
            return;
        }

        // Sort by filename (case-insensitive)
        const sorted = attachments.slice().sort(function (a, b) {
            return a.filename.toLowerCase().localeCompare(b.filename.toLowerCase());
        });

        let html = '';
        for (let i = 0; i < sorted.length; i++) {
            html += this.renderItem(sorted[i], i);
        }
        this.listEl.innerHTML = html;
    },

    renderItem: function (att, index) {
        const icon = att.thumbnailUrl
            ? '<img class="attachment-thumb" src="' + att.thumbnailUrl + '" alt="">'
            : '<span class="attachment-icon">' + this.getTypeIcon(att.filename) + '</span>';

        return '<div class="attachment-item" tabindex="-1" data-index="' + index + '" data-filename="' + this.escapeHtml(att.filename) + '">' +
            icon +
            '<span class="attachment-name">' + this.escapeHtml(att.filename) + '</span>' +
            '<span class="attachment-size">' + this.formatSize(att.size) + '</span>' +
            '<span class="attachment-actions">' +
            '<button class="attachment-action-btn" data-action="rename" title="Rename">\u270F\uFE0F</button>' +
            '<button class="attachment-action-btn" data-action="delete" title="Delete">\u2716</button>' +
            '</span>' +
            '</div>';
    },

    formatSize: function (bytes) {
        if (bytes < 1024) return bytes + ' B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
        if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
        return (bytes / (1024 * 1024 * 1024)).toFixed(1) + ' GB';
    },

    getTypeIcon: function (filename) {
        const ext = (filename.split('.').pop() || '').toLowerCase();
        const icon = {
            document: '\uD83D\uDCC4',
            chart: '\uD83D\uDCCA',
            image: '\uD83D\uDDBC\uFE0F',
            audio: '\uD83C\uDFB5',
            video: '\uD83C\uDFAC',
            archive: '\uD83D\uDCE6',
            file: '\uD83D\uDCC1',
        };
        const extToIcon = {
            pdf: icon.document,
            doc: icon.document, docx: icon.document,
            xls: icon.chart, xlsx: icon.chart,
            ppt: icon.chart, pptx: icon.chart,
            txt: icon.document, md: icon.document,
            png: icon.image, jpg: icon.image, jpeg: icon.image,
            gif: icon.image, bmp: icon.image, webp: icon.image,
            svg: icon.image,
            mp3: icon.audio, wav: icon.audio, ogg: icon.audio, flac: icon.audio,
            mp4: icon.video, avi: icon.video, mkv: icon.video, mov: icon.video,
            zip: icon.archive, rar: icon.archive, '7z': icon.archive, tar: icon.archive, gz: icon.archive,
        };
        return extToIcon[ext] || icon.file;
    },

    escapeHtml: function (str) {
        return str.replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;');
    },

    getItems: function () {
        return this.dom.container.querySelectorAll('.attachment-item');
    },

    singleSelect: function (index) {
        this.selectedIndices.clear();
        this.selectedIndices.add(index);
        this.updateSelectionClasses();
    },

    toggleSelect: function (index) {
        if (this.selectedIndices.has(index)) {
            this.selectedIndices.delete(index);
        } else {
            this.selectedIndices.add(index);
        }
        this.updateSelectionClasses();
    },

    rangeSelect: function (fromIndex, toIndex) {
        const start = Math.min(fromIndex, toIndex);
        const end = Math.max(fromIndex, toIndex);
        this.selectedIndices.clear();
        for (let i = start; i <= end; i++) {
            this.selectedIndices.add(i);
        }
        this.updateSelectionClasses();
    },

    navigateArrow: function (direction, shiftKey) {
        const items = this.getItems();
        if (items.length === 0) return;

        const focused = document.activeElement;
        let currentIndex = Array.prototype.indexOf.call(items, focused);
        if (currentIndex === -1) currentIndex = 0;

        const newIndex = currentIndex + direction;
        if (newIndex < 0 || newIndex >= items.length) return;

        if (shiftKey) {
            if (this.lastClickedIndex === -1) {
                this.lastClickedIndex = currentIndex;
            }
            this.rangeSelect(this.lastClickedIndex, newIndex);
        } else {
            this.singleSelect(newIndex);
            this.lastClickedIndex = newIndex;
        }

        items[newIndex].focus();
    },

    updateSelectionClasses: function () {
        const items = this.getItems();
        for (let i = 0; i < items.length; i++) {
            if (this.selectedIndices.has(i)) {
                items[i].classList.add('selected');
            } else {
                items[i].classList.remove('selected');
            }
        }
    },

    clearSelection: function () {
        this.selectedIndices.clear();
        this.lastClickedIndex = -1;
        this.updateSelectionClasses();
    },

    getSelectedItems: function () {
        const items = this.getItems();
        const selected = [];
        this.selectedIndices.forEach(function (index) {
            if (items[index]) {
                selected.push(items[index]);
            }
        });
        return selected;
    },

    // Check if filename is referenced in markdown content
    isReferenced: function (filename) {
        if (!Editor.instance) return false;
        const content = Editor.instance.getValue();
        // Match [any text](att:filename)
        const escaped = filename.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const pattern = new RegExp('\\]\\(att:' + escaped + '\\)', 'i');
        return pattern.test(content);
    },

    // Rename operations
    startRename: function (filename) {
        this.cancelRename();

        const item = this.dom.container.querySelector('.attachment-item[data-filename="' + CSS.escape(filename) + '"]');
        if (!item) return;

        this.renameState = {
            oldFilename: filename,
            element: item
        };

        item.classList.add('editing');
        item.style.position = 'relative';

        const input = document.createElement('input');
        input.type = 'text';
        input.className = 'rename-input';
        input.value = filename;
        item.appendChild(input);

        input.focus();
        // Select filename without extension
        const lastDot = filename.lastIndexOf('.');
        if (lastDot > 0) {
            input.setSelectionRange(0, lastDot);
        } else {
            input.select();
        }

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
            // Small delay to allow button clicks to fire
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

        const newFilename = input.value.trim();
        const oldFilename = this.renameState.oldFilename;

        // Clear any existing error
        this.clearRenameError();

        // Validate
        if (!newFilename) {
            this.showRenameError('Filename cannot be empty');
            input.focus();
            return;
        }

        if (newFilename === oldFilename) {
            this.cancelRename();
            return;
        }

        // Check for duplicates
        const existingNames = new Set();
        for (let i = 0; i < State.data.attachments.length; i++) {
            if (State.data.attachments[i].filename !== oldFilename) {
                existingNames.add(State.data.attachments[i].filename);
            }
        }

        if (existingNames.has(newFilename)) {
            // Show conflict dialog
            this.showRenameConflictDialog(oldFilename, newFilename);
            return;
        }

        // Check for references
        if (this.isReferenced(oldFilename)) {
            this.showRenameReferenceDialog(oldFilename, newFilename, false);
            return;
        }

        // No conflicts, no references - proceed with rename
        this.doRename(oldFilename, newFilename, false, false);
    },

    doRename: function (oldFilename, newFilename, updateReferences, force) {
        this.cancelRename();

        // Update references in editor if requested (frontend handles this)
        if (updateReferences && Editor.instance) {
            const content = Editor.instance.getValue();
            const oldPattern = '](att:' + oldFilename + ')';
            const newPattern = '](att:' + newFilename + ')';
            const updated = content.split(oldPattern).join(newPattern);
            if (updated !== content) {
                Editor.instance.setValue(updated);
            }
        }

        Api.send({
            type: 'renameAttachment',
            key: State.data.selectedKey,
            oldFilename: oldFilename,
            newFilename: newFilename,
            force: force
        });
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

    // Helper for Tab cycling in dialogs
    handleDialogTab: function (e, buttons) {
        if (e.key !== 'Tab') return;
        e.preventDefault();

        const currentIndex = buttons.indexOf(document.activeElement);
        let nextIndex;
        if (e.shiftKey) {
            nextIndex = currentIndex <= 0 ? buttons.length - 1 : currentIndex - 1;
        } else {
            nextIndex = currentIndex >= buttons.length - 1 ? 0 : currentIndex + 1;
        }
        buttons[nextIndex].focus();
    },

    showRenameConflictDialog: function (oldFilename, newFilename) {
        const self = this;
        const overlay = document.createElement('div');
        overlay.className = 'dialog-overlay';

        const dialog = document.createElement('div');
        dialog.className = 'dialog';
        dialog.tabIndex = -1;

        const message = document.createElement('div');
        message.className = 'dialog-message';
        message.textContent = '"' + newFilename + '" already exists. Overwrite?';

        const buttons = document.createElement('div');
        buttons.className = 'dialog-buttons';

        const cancelBtn = document.createElement('button');
        cancelBtn.className = 'dialog-btn dialog-btn-secondary';
        cancelBtn.textContent = 'Cancel';
        cancelBtn.onclick = function () {
            overlay.remove();
            self.cancelRename();
        };

        const overwriteBtn = document.createElement('button');
        overwriteBtn.className = 'dialog-btn dialog-btn-primary';
        overwriteBtn.textContent = 'Overwrite';
        overwriteBtn.onclick = function () {
            overlay.remove();
            // Check for references before overwrite
            if (self.isReferenced(oldFilename)) {
                self.showRenameReferenceDialog(oldFilename, newFilename, true);
            } else {
                self.doRename(oldFilename, newFilename, false, true);
            }
        };

        buttons.appendChild(cancelBtn);
        buttons.appendChild(overwriteBtn);
        dialog.appendChild(message);
        dialog.appendChild(buttons);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);

        const focusableButtons = [cancelBtn, overwriteBtn];
        overwriteBtn.focus();

        overlay.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                e.stopPropagation();
                overlay.remove();
                self.cancelRename();
            } else if (e.key === 'Tab') {
                self.handleDialogTab(e, focusableButtons);
            }
        });

        // Restore keyboard focus without selecting a button
        overlay.addEventListener('click', function (e) {
            if (!e.target.closest('button')) {
                dialog.focus();
            }
        });
    },

    showRenameReferenceDialog: function (oldFilename, newFilename, force) {
        const self = this;
        const overlay = document.createElement('div');
        overlay.className = 'dialog-overlay';

        const dialog = document.createElement('div');
        dialog.className = 'dialog';
        dialog.tabIndex = -1;

        const message = document.createElement('div');
        message.className = 'dialog-message';
        message.textContent = '"' + oldFilename + '" is referenced in your notes. Update references to "' + newFilename + '"?';

        const buttons = document.createElement('div');
        buttons.className = 'dialog-buttons';

        const cancelBtn = document.createElement('button');
        cancelBtn.className = 'dialog-btn dialog-btn-secondary';
        cancelBtn.textContent = 'Cancel';
        cancelBtn.onclick = function () {
            overlay.remove();
            self.cancelRename();
        };

        const dontUpdateBtn = document.createElement('button');
        dontUpdateBtn.className = 'dialog-btn dialog-btn-secondary';
        dontUpdateBtn.textContent = "Don't Update";
        dontUpdateBtn.onclick = function () {
            overlay.remove();
            self.doRename(oldFilename, newFilename, false, force);
        };

        const updateBtn = document.createElement('button');
        updateBtn.className = 'dialog-btn dialog-btn-primary';
        updateBtn.textContent = 'Update';
        updateBtn.onclick = function () {
            overlay.remove();
            self.doRename(oldFilename, newFilename, true, force);
        };

        buttons.appendChild(cancelBtn);
        buttons.appendChild(dontUpdateBtn);
        buttons.appendChild(updateBtn);
        dialog.appendChild(message);
        dialog.appendChild(buttons);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);

        const focusableButtons = [cancelBtn, dontUpdateBtn, updateBtn];
        updateBtn.focus();

        overlay.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                e.stopPropagation();
                overlay.remove();
                self.cancelRename();
            } else if (e.key === 'Tab') {
                self.handleDialogTab(e, focusableButtons);
            }
        });

        // Restore keyboard focus without selecting a button
        overlay.addEventListener('click', function (e) {
            if (!e.target.closest('button')) {
                dialog.focus();
            }
        });
    },

    // Delete operations
    startDelete: function (filename) {
        this.showDeleteConfirmDialog(filename);
    },

    doDelete: function (filename) {
        Api.send({
            type: 'removeAttachment',
            key: State.data.selectedKey,
            filename: filename
        });
    },

    showDeleteConfirmDialog: function (filename) {
        const self = this;
        const overlay = document.createElement('div');
        overlay.className = 'dialog-overlay';

        const dialog = document.createElement('div');
        dialog.className = 'dialog';
        dialog.tabIndex = -1;

        const message = document.createElement('div');
        message.className = 'dialog-message';
        message.textContent = 'Delete "' + filename + '"?';

        const buttons = document.createElement('div');
        buttons.className = 'dialog-buttons';

        const cancelBtn = document.createElement('button');
        cancelBtn.className = 'dialog-btn dialog-btn-secondary';
        cancelBtn.textContent = 'Cancel';
        cancelBtn.onclick = function () {
            overlay.remove();
        };

        const deleteBtn = document.createElement('button');
        deleteBtn.className = 'dialog-btn dialog-btn-primary';
        deleteBtn.textContent = 'Delete';
        deleteBtn.style.background = '#f44336';
        deleteBtn.onclick = function () {
            overlay.remove();
            self.doDelete(filename);
        };

        buttons.appendChild(cancelBtn);
        buttons.appendChild(deleteBtn);
        dialog.appendChild(message);
        dialog.appendChild(buttons);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);

        const focusableButtons = [cancelBtn, deleteBtn];
        cancelBtn.focus();

        overlay.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                e.stopPropagation();
                overlay.remove();
            } else if (e.key === 'Tab') {
                self.handleDialogTab(e, focusableButtons);
            }
        });

        // Restore keyboard focus without selecting a button
        overlay.addEventListener('click', function (e) {
            if (!e.target.closest('button')) {
                dialog.focus();
            }
        });
    }
};
