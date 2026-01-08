'use strict';

// Reusable conflict dialog for file operations (drop, clipboard paste, file picker)
const ConflictDialog = {
    /// Partitions files into conflicts/nonConflicts based on existing attachments.
    /// fileList: array of {id, filename} where id is index or path
    /// Returns: {conflicts: [[id, filename], ...], nonConflicts: [[id, filename], ...]}
    checkConflicts: function (fileList) {
        const existingNames = new Set();
        for (let i = 0; i < State.data.attachments.length; i++) {
            existingNames.add(State.data.attachments[i].filename);
        }

        const conflicts = [];
        const nonConflicts = [];
        for (let i = 0; i < fileList.length; i++) {
            const file = fileList[i];
            if (existingNames.has(file.filename)) {
                conflicts.push([file.id, file.filename]);
            } else {
                nonConflicts.push([file.id, file.filename]);
            }
        }
        return { conflicts: conflicts, nonConflicts: nonConflicts };
    },

    overlay: null,
    key: null,
    conflicts: [],
    pending: [],
    takenNames: null,
    applyToAll: false,
    resolutions: [],
    insertLinks: false,
    editorPosition: null,
    onFinish: null,

    show: function (key, conflicts, nonConflicts, insertLinks, editorPosition, onFinish) {
        this.key = key;
        this.conflicts = conflicts.slice();
        this.pending = nonConflicts.slice();
        this.applyToAll = false;
        this.resolutions = [];
        this.insertLinks = insertLinks;
        this.editorPosition = editorPosition;
        this.onFinish = onFinish;

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
            '    <input type="checkbox" id="conflict-apply-to-all"> Apply to all' +
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

        this.overlay.querySelector('#conflict-apply-to-all').addEventListener('change', function () {
            self.applyToAll = this.checked;
        });

        var checkbox = this.overlay.querySelector('#conflict-apply-to-all');

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

        if (this.onFinish) {
            this.onFinish(this.key, this.resolutions, this.insertLinks, this.editorPosition);
        }
    }
};
