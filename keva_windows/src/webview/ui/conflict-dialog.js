'use strict';

const ConflictDialog = {
    overlay: null,
    key: null,
    conflicts: [],
    pending: [],      // Files not yet resolved: [path, filename]
    takenNames: null, // Set of filenames that are taken
    applyToAll: false,
    resolutions: [],

    show: function (key, conflicts, nonConflicts) {
        this.key = key;
        this.conflicts = conflicts.slice();
        this.pending = nonConflicts.slice();
        this.applyToAll = false;
        this.resolutions = [];

        // Build initial set of taken filenames from existing attachments only.
        // Pending files are NOT included - they may become conflicts as we resolve.
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
            '<div class="conflict-dialog">' +
            '  <div class="conflict-title">File already exists</div>' +
            '  <div class="conflict-filename"></div>' +
            '  <div class="conflict-buttons">' +
            '    <button class="conflict-btn" data-action="overwrite">Overwrite</button>' +
            '    <button class="conflict-btn" data-action="rename">Keep Both</button>' +
            '    <button class="conflict-btn" data-action="skip">Skip</button>' +
            '  </div>' +
            '  <label class="conflict-apply-all">' +
            '    <input type="checkbox" id="apply-to-all"> Apply to all' +
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

        this.overlay.querySelector('#apply-to-all').addEventListener('change', function () {
            self.applyToAll = this.checked;
        });
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
            // Apply to all remaining conflicts
            for (let i = 0; i < this.conflicts.length; i++) {
                this.resolveFile(this.conflicts[i], action);
            }
            this.conflicts = [];
        } else {
            // Apply to current conflict only
            const conflict = this.conflicts.shift();
            this.resolveFile(conflict, action);
        }

        // Check if any pending files now conflict
        this.recheckPending();
        this.showCurrentConflict();
    },

    resolveFile: function (file, action) {
        const path = file[0];
        const filename = file[1];

        if (action === 'skip') {
            // Don't add to resolutions
            return;
        }

        if (action === 'rename') {
            const newName = this.findNextAvailableName(filename);
            this.resolutions.push([path, newName]);
            this.takenNames.add(newName);
        } else if (action === 'overwrite') {
            this.resolutions.push([path, filename]);
            // Filename remains taken (overwriting existing)
        }
    },

    findNextAvailableName: function (filename) {
        // Frontend is the source of truth for rename decisions
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
        // Move any pending files that now conflict to the conflicts list
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

    finish: function () {
        if (this.overlay) {
            this.overlay.remove();
            this.overlay = null;
        }

        // Add remaining pending files with their original filenames
        for (let i = 0; i < this.pending.length; i++) {
            const path = this.pending[i][0];
            const filename = this.pending[i][1];
            this.resolutions.push([path, filename]);
        }

        // Skip sending if all files were skipped
        if (this.resolutions.length === 0) {
            return;
        }

        State.data.isCopying = true;
        Main.showAddingOverlay();

        Api.send({
            type: 'addAttachments',
            key: this.key,
            files: this.resolutions
        });
    }
};
