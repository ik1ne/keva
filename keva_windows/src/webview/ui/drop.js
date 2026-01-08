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
            return {type: 'keyItem', key: keyItem.dataset.key};
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

        // Build file list and check for conflicts
        const fileList = [];
        for (let i = 0; i < files.length; i++) {
            fileList.push({ id: i, filename: files[i].name });
        }
        const result = ConflictDialog.checkConflicts(fileList);

        // Store drop context for link insertion
        const insertLinks = (dropTarget === 'editor');
        const editorPosition = insertLinks ? this.getEditorDropPosition(e) : null;

        if (result.conflicts.length > 0) {
            ConflictDialog.show(targetKey, result.conflicts, result.nonConflicts, insertLinks, editorPosition, Drop.sendDroppedFiles.bind(Drop));
        } else {
            this.sendDroppedFiles(targetKey, result.nonConflicts, insertLinks, editorPosition);
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
        const endPos = {lineNumber: endLine, column: endColumn};

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

        // Store pending link insertion info (position converted to selection format)
        if (insertLinks && editorPosition) {
            State.data.pendingLinkInsert = {
                files: files.map(function (f) {
                    return f[1];
                }), // filenames
                selection: editorPosition // insertAttachmentLinks handles both formats
            };
        }

        Api.send({
            type: 'addDroppedFiles',
            key: key,
            files: files // [[index, filename], ...]
        });
    },

    // Accepts either a position {lineNumber, column} or selection {startLineNumber, startColumn, endLineNumber, endColumn}
    insertAttachmentLinks: function (filenames, positionOrSelection) {
        if (!Editor.instance || !positionOrSelection || filenames.length === 0) return;

        const links = filenames.map(function (filename) {
            var prefix = Editor.isImageFile(filename) ? '!' : '';
            return prefix + '[' + filename + '](att:' + encodeURIComponent(filename) + ')';
        }).join(', ');

        // 1. Focus window and editor FIRST
        window.focus();
        Editor.instance.focus();

        // 2. Build range - selection replaces selected text, position inserts at point
        var range;
        if (positionOrSelection.startLineNumber !== undefined) {
            // Selection object from getSelection()
            range = {
                startLineNumber: positionOrSelection.startLineNumber,
                startColumn: positionOrSelection.startColumn,
                endLineNumber: positionOrSelection.endLineNumber,
                endColumn: positionOrSelection.endColumn
            };
        } else {
            // Position object from getEditorDropPosition()
            range = {
                startLineNumber: positionOrSelection.lineNumber,
                startColumn: positionOrSelection.column,
                endLineNumber: positionOrSelection.lineNumber,
                endColumn: positionOrSelection.column
            };
        }

        // 3. Insert/replace content
        Editor.instance.executeEdits('drop', [{
            range: range,
            text: links
        }]);

        // 4. Move cursor to end of inserted content
        const endColumn = range.startColumn + links.length;
        const endPos = { lineNumber: range.startLineNumber, column: endColumn };

        Editor.instance.setPosition(endPos);

        // 5. Reveal the position
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
