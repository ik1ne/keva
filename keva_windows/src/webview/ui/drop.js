'use strict';

const Drop = {
    dropCursorDecorations: [],
    isDragging: false,

    init: function () {
        this.setupEventHandlers();
    },

    setupEventHandlers: function () {
        const self = this;

        // Prevent default drag behavior on document
        document.addEventListener('dragenter', function (e) {
            e.preventDefault();
            self.isDragging = true;
        });

        document.addEventListener('dragover', function (e) {
            e.preventDefault();
            // Set drop effect based on target
            if (self.isValidDropTarget(e)) {
                e.dataTransfer.dropEffect = 'copy';
                // Show cursor preview in editor
                self.showDropCursor(e);
            } else {
                e.dataTransfer.dropEffect = 'none';
            }
        });

        document.addEventListener('dragleave', function (e) {
            e.preventDefault();
            // Only clear if leaving the document entirely
            if (e.relatedTarget === null) {
                self.isDragging = false;
                self.clearDropCursor();
            }
        });

        document.addEventListener('drop', function (e) {
            e.preventDefault();
            self.isDragging = false;
            self.clearDropCursor();
            self.handleDrop(e);
        });
    },

    isValidDropTarget: function (e) {
        if (!e.dataTransfer) return false;

        const types = e.dataTransfer.types;

        // Internal attachment drag - always allow if dropping on editor
        if (types.includes('application/x-keva-attachments')) {
            return true;
        }

        // Text drop on editor - always allow
        if (types.includes('text/plain') || types.includes('text/html')) {
            return true;
        }

        // File drops require a key selected and not trashed
        if (types.includes('Files')) {
            if (!State.data.selectedKey) return false;
            if (State.data.isSelectedTrashed) return false;
            return true;
        }

        return false;
    },

    getDropTarget: function (e) {
        // Check if drop is on Monaco editor
        const editorContainer = document.getElementById('editor-container');
        if (editorContainer && editorContainer.contains(e.target)) {
            return 'editor';
        }

        // Check if drop is on attachments pane
        const attachments = document.getElementById('attachments-container');
        if (attachments && attachments.contains(e.target)) {
            return 'attachments';
        }

        // Check if drop is on a key item in the list
        const keyItem = e.target.closest('.key-item');
        if (keyItem) {
            return { type: 'keyItem', key: keyItem.dataset.key };
        }

        // Default - treat as attachments pane drop
        return 'attachments';
    },

    handleDrop: function (e) {
        // Check for internal attachment drag (from attachments pane to Monaco)
        const attachmentData = e.dataTransfer.getData('application/x-keva-attachments');
        if (attachmentData) {
            this.handleInternalAttachmentDrop(e, attachmentData);
            return;
        }

        // Check for text drop on editor
        const textData = e.dataTransfer.getData('text/plain');
        if (textData && this.getDropTarget(e) === 'editor') {
            this.handleTextDrop(e, textData);
            return;
        }

        // Reject file drops if no key selected or trashed
        if (!State.data.selectedKey) return;
        if (State.data.isSelectedTrashed) {
            this.showToast('Cannot add files to trashed key');
            return;
        }

        const files = e.dataTransfer.files;
        if (!files || files.length === 0) return;

        const dropTarget = this.getDropTarget(e);
        const targetKey = State.data.selectedKey;

        // Handle drop on different key (future feature)
        if (typeof dropTarget === 'object' && dropTarget.type === 'keyItem') {
            // For now, only handle drops on selected key
            if (dropTarget.key !== targetKey) {
                this.showToast('Select the key first to add files');
                return;
            }
        }

        // Build file list with indices
        const fileList = [];
        for (let i = 0; i < files.length; i++) {
            fileList.push({ index: i, filename: files[i].name });
        }

        // Check for conflicts with existing attachments
        const existingNames = new Set();
        for (let i = 0; i < State.data.attachments.length; i++) {
            existingNames.add(State.data.attachments[i].filename);
        }

        const conflicts = [];
        const nonConflicts = [];
        for (let i = 0; i < fileList.length; i++) {
            const file = fileList[i];
            if (existingNames.has(file.filename)) {
                conflicts.push([file.index, file.filename]);
            } else {
                nonConflicts.push([file.index, file.filename]);
            }
        }

        // Store drop context for link insertion
        const insertLinks = (dropTarget === 'editor');
        const editorPosition = insertLinks ? this.getEditorDropPosition(e) : null;

        if (conflicts.length > 0) {
            DropConflictDialog.show(targetKey, conflicts, nonConflicts, insertLinks, editorPosition);
        } else {
            this.sendDroppedFiles(targetKey, nonConflicts, insertLinks, editorPosition);
        }
    },

    handleInternalAttachmentDrop: function (e, attachmentData) {
        const dropTarget = this.getDropTarget(e);
        if (dropTarget !== 'editor') {
            // Internal attachment drop only makes sense on editor
            return;
        }

        const position = this.getEditorDropPosition(e);
        if (!position) return;

        try {
            const data = JSON.parse(attachmentData);
            if (data.filenames && data.filenames.length > 0) {
                this.insertAttachmentLinks(data.filenames, position);
            }
        } catch (err) {
            console.error('[Drop] Failed to parse attachment data:', err);
        }
    },

    handleTextDrop: function (e, textData) {
        const position = this.getEditorDropPosition(e);
        if (!position || !Editor.instance) return;

        // 1. Focus window and editor FIRST
        window.focus();
        Editor.instance.focus();

        // 2. Insert content at position
        Editor.instance.executeEdits('drop', [{
            range: {
                startLineNumber: position.lineNumber,
                startColumn: position.column,
                endLineNumber: position.lineNumber,
                endColumn: position.column
            },
            text: textData
        }]);

        // 3. Move cursor to end of inserted content
        const lines = textData.split('\n');
        const endLine = position.lineNumber + lines.length - 1;
        const endColumn = lines.length === 1
            ? position.column + textData.length
            : lines[lines.length - 1].length + 1;
        const endPos = { lineNumber: endLine, column: endColumn };

        Editor.instance.setPosition(endPos);

        // 4. Reveal the position
        Editor.instance.revealPositionInCenter(endPos);
    },

    showDropCursor: function (e) {
        if (this.getDropTarget(e) !== 'editor') {
            this.clearDropCursor();
            return;
        }
        if (!Editor.instance) return;

        const target = Editor.instance.getTargetAtClientPoint(e.clientX, e.clientY);
        if (target && target.position) {
            const pos = target.position;
            this.dropCursorDecorations = Editor.instance.deltaDecorations(
                this.dropCursorDecorations,
                [{
                    range: {
                        startLineNumber: pos.lineNumber,
                        startColumn: pos.column,
                        endLineNumber: pos.lineNumber,
                        endColumn: pos.column
                    },
                    options: {
                        className: 'drop-cursor-line',
                        beforeContentClassName: 'drop-cursor-marker'
                    }
                }]
            );
        }
    },

    clearDropCursor: function () {
        if (Editor.instance && this.dropCursorDecorations.length > 0) {
            this.dropCursorDecorations = Editor.instance.deltaDecorations(
                this.dropCursorDecorations,
                []
            );
        }
    },

    getEditorDropPosition: function (e) {
        if (!Editor.instance) return null;

        const target = Editor.instance.getTargetAtClientPoint(e.clientX, e.clientY);
        if (target && target.position) {
            return target.position;
        }
        return null;
    },

    sendDroppedFiles: function (key, files, insertLinks, editorPosition) {
        if (files.length === 0) return;

        State.data.isCopying = true;
        Main.showAddingOverlay();

        // Store pending link insertion info
        if (insertLinks && editorPosition) {
            State.data.pendingLinkInsert = {
                files: files.map(function (f) { return f[1]; }), // filenames
                position: editorPosition
            };
        }

        Api.send({
            type: 'addDroppedFiles',
            key: key,
            files: files // [[index, filename], ...]
        });
    },

    insertAttachmentLinks: function (filenames, position) {
        if (!Editor.instance || !position || filenames.length === 0) return;

        const links = filenames.map(function (filename) {
            var prefix = Editor.isImageFile(filename) ? '!' : '';
            return prefix + '[' + filename + '](att:' + encodeURIComponent(filename) + ')';
        }).join(', ');

        // 1. Focus window and editor FIRST
        window.focus();
        Editor.instance.focus();

        // 2. Insert content at position
        Editor.instance.executeEdits('drop', [{
            range: {
                startLineNumber: position.lineNumber,
                startColumn: position.column,
                endLineNumber: position.lineNumber,
                endColumn: position.column
            },
            text: links
        }]);

        // 3. Move cursor to end of inserted content
        const endColumn = position.column + links.length;
        const endPos = { lineNumber: position.lineNumber, column: endColumn };

        Editor.instance.setPosition(endPos);

        // 4. Reveal the position
        Editor.instance.revealPositionInCenter(endPos);
    },

    showToast: function (message) {
        // Simple toast notification
        const toast = document.createElement('div');
        toast.className = 'toast';
        toast.textContent = message;
        document.body.appendChild(toast);

        setTimeout(function () {
            toast.classList.add('fade-out');
            setTimeout(function () {
                toast.remove();
            }, 300);
        }, 2000);
    }
};

// Conflict dialog for dropped files (uses indices instead of paths)
const DropConflictDialog = {
    overlay: null,
    key: null,
    conflicts: [],
    pending: [],
    takenNames: null,
    applyToAll: false,
    resolutions: [],
    insertLinks: false,
    editorPosition: null,

    show: function (key, conflicts, nonConflicts, insertLinks, editorPosition) {
        this.key = key;
        this.conflicts = conflicts.slice();
        this.pending = nonConflicts.slice();
        this.applyToAll = false;
        this.resolutions = [];
        this.insertLinks = insertLinks;
        this.editorPosition = editorPosition;

        // Build initial set of taken filenames
        this.takenNames = new Set();
        for (let i = 0; i < State.data.attachments.length; i++) {
            this.takenNames.add(State.data.attachments[i].filename);
        }

        this.createOverlay();
        this.showCurrentConflict();
    },

    createOverlay: function () {
        if (this.overlay) {
            this.overlay.remove();
        }

        this.overlay = document.createElement('div');
        this.overlay.className = 'conflict-overlay';
        this.overlay.innerHTML =
            '<div class="conflict-dialog" tabindex="-1">' +
            '  <div class="conflict-title">File already exists</div>' +
            '  <div class="conflict-filename"></div>' +
            '  <div class="conflict-buttons">' +
            '    <button class="conflict-btn" data-action="overwrite">Overwrite</button>' +
            '    <button class="conflict-btn" data-action="rename">Keep Both</button>' +
            '    <button class="conflict-btn" data-action="skip">Skip</button>' +
            '  </div>' +
            '  <label class="conflict-apply-all">' +
            '    <input type="checkbox" id="drop-apply-to-all"> Apply to all' +
            '  </label>' +
            '</div>';

        document.body.appendChild(this.overlay);

        const self = this;
        const buttons = this.overlay.querySelectorAll('.conflict-btn');
        for (let i = 0; i < buttons.length; i++) {
            buttons[i].addEventListener('click', function () {
                self.handleAction(this.dataset.action);
            });
        }

        this.overlay.querySelector('#drop-apply-to-all').addEventListener('change', function () {
            self.applyToAll = this.checked;
        });

        var checkbox = this.overlay.querySelector('#drop-apply-to-all');

        this.overlay.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                e.stopPropagation();
                self.cancel();
            } else if (e.key === 'Tab') {
                e.preventDefault();
                var focusables = Array.prototype.slice.call(buttons);
                if (checkbox.offsetParent !== null) {
                    focusables.push(checkbox);
                }
                var currentIndex = focusables.indexOf(document.activeElement);
                var nextIndex;
                if (e.shiftKey) {
                    nextIndex = currentIndex <= 0 ? focusables.length - 1 : currentIndex - 1;
                } else {
                    nextIndex = currentIndex >= focusables.length - 1 ? 0 : currentIndex + 1;
                }
                focusables[nextIndex].focus();
            }
        });

        var dialog = this.overlay.querySelector('.conflict-dialog');
        this.overlay.addEventListener('click', function (e) {
            if (!e.target.closest('button') && !e.target.closest('input')) {
                dialog.focus();
            }
        });

        buttons[0].focus();
    },

    showCurrentConflict: function () {
        if (this.conflicts.length === 0) {
            this.finish();
            return;
        }

        const conflict = this.conflicts[0];
        const filename = conflict[1];

        this.overlay.querySelector('.conflict-filename').textContent =
            '"' + filename + '" already exists.';

        const applyAllLabel = this.overlay.querySelector('.conflict-apply-all');
        if (this.conflicts.length > 1) {
            applyAllLabel.style.display = 'block';
            applyAllLabel.querySelector('input').nextSibling.textContent =
                ' Apply to all (' + this.conflicts.length + ' remaining)';
        } else {
            applyAllLabel.style.display = 'none';
        }
    },

    handleAction: function (action) {
        if (this.applyToAll) {
            for (let i = 0; i < this.conflicts.length; i++) {
                this.resolveFile(this.conflicts[i], action);
            }
            this.conflicts = [];
        } else {
            const conflict = this.conflicts.shift();
            this.resolveFile(conflict, action);
        }

        this.recheckPending();
        this.showCurrentConflict();
    },

    resolveFile: function (file, action) {
        const index = file[0];
        const filename = file[1];

        if (action === 'skip') {
            return;
        }

        if (action === 'rename') {
            const newName = this.findNextAvailableName(filename);
            this.resolutions.push([index, newName]);
            this.takenNames.add(newName);
        } else if (action === 'overwrite') {
            this.resolutions.push([index, filename]);
        }
    },

    findNextAvailableName: function (filename) {
        const dotIndex = filename.lastIndexOf('.');
        let stem, ext;
        if (dotIndex > 0) {
            stem = filename.substring(0, dotIndex);
            ext = filename.substring(dotIndex);
        } else {
            stem = filename;
            ext = '';
        }

        let counter = 1;
        let newName;
        do {
            newName = stem + ' (' + counter + ')' + ext;
            counter++;
        } while (this.takenNames.has(newName));

        return newName;
    },

    recheckPending: function () {
        const stillPending = [];
        for (let i = 0; i < this.pending.length; i++) {
            const file = this.pending[i];
            if (this.takenNames.has(file[1])) {
                this.conflicts.push(file);
            } else {
                stillPending.push(file);
            }
        }
        this.pending = stillPending;
    },

    cancel: function () {
        if (this.overlay) {
            this.overlay.remove();
            this.overlay = null;
        }
    },

    finish: function () {
        if (this.overlay) {
            this.overlay.remove();
            this.overlay = null;
        }

        // Add remaining pending files
        for (let i = 0; i < this.pending.length; i++) {
            const index = this.pending[i][0];
            const filename = this.pending[i][1];
            this.resolutions.push([index, filename]);
        }

        if (this.resolutions.length === 0) {
            return;
        }

        Drop.sendDroppedFiles(this.key, this.resolutions, this.insertLinks, this.editorPosition);
    }
};
