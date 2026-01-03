'use strict';

const Attachments = {
    dom: null,
    selectedIndices: new Set(),
    lastClickedIndex: -1,

    init: function (dom) {
        this.dom = dom;
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
