'use strict';

const Attachments = {
    dom: null,
    listEl: null,
    selectedIndices: new Set(),
    lastClickedIndex: -1,

    init: function (dom) {
        this.dom = dom;
        this.listEl = document.getElementById('attachments-list');
        this.setupEventHandlers();
    },

    setupEventHandlers: function () {
        const self = this;
        if (!this.dom.container) return;

        this.dom.container.addEventListener('click', function (e) {
            const item = e.target.closest('.attachment-item');
            if (!item) return;

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

        return '<div class="attachment-item" tabindex="-1" data-index="' + index + '">' +
            icon +
            '<span class="attachment-name">' + this.escapeHtml(att.filename) + '</span>' +
            '<span class="attachment-size">' + this.formatSize(att.size) + '</span>' +
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
    }
};
