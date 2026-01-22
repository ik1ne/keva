'use strict';

export const Resizer = {
    leftPane: null,
    dividerV: null,
    dividerH: null,
    attachments: null,
    editorViewport: null,
    rightPane: null,

    isDraggingV: false,
    isDraggingH: false,
    startX: 0,
    startY: 0,
    startWidth: 0,
    startHeight: 0,

    // Constraints
    MIN_LEFT_WIDTH: 150,
    MAX_LEFT_RATIO: 0.5,
    MIN_ATTACHMENTS_HEIGHT: 100,
    MAX_ATTACHMENTS_RATIO: 0.5,

    init: function (opts) {
        this.leftPane = opts.leftPane;
        this.dividerV = opts.dividerV;
        this.dividerH = opts.dividerH;
        this.attachments = opts.attachments;
        this.editorViewport = opts.editorViewport;
        this.rightPane = opts.rightPane;

        this.setupEventHandlers();
        this.setupWindowResize();
    },

    setupEventHandlers: function () {
        const self = this;

        // Vertical divider (left/right panes)
        this.dividerV.addEventListener('mousedown', function (e) {
            if (e.button !== 0) return;
            e.preventDefault();
            self.startDragV(e.clientX);
        });

        // Horizontal divider (editor/attachments)
        this.dividerH.addEventListener('mousedown', function (e) {
            if (e.button !== 0) return;
            e.preventDefault();
            self.startDragH(e.clientY);
        });

        // Global mouse events for drag tracking
        document.addEventListener('mousemove', function (e) {
            if (self.isDraggingV) {
                self.onDragV(e.clientX);
            } else if (self.isDraggingH) {
                self.onDragH(e.clientY);
            }
        });

        document.addEventListener('mouseup', function () {
            if (self.isDraggingV) {
                self.endDragV();
            } else if (self.isDraggingH) {
                self.endDragH();
            }
        });
    },

    setupWindowResize: function () {
        const self = this;

        window.addEventListener('resize', function () {
            self.clampSizes();
        });
    },

    startDragV: function (clientX) {
        this.isDraggingV = true;
        this.startX = clientX;
        this.startWidth = this.leftPane.offsetWidth;
        this.dividerV.classList.add('dragging');
        document.body.classList.add('resizing');
    },

    onDragV: function (clientX) {
        const delta = clientX - this.startX;
        const newWidth = this.startWidth + delta;
        const containerWidth = this.leftPane.parentElement.offsetWidth;
        const maxWidth = containerWidth * this.MAX_LEFT_RATIO;

        const clampedWidth = Math.max(this.MIN_LEFT_WIDTH, Math.min(maxWidth, newWidth));
        this.leftPane.style.width = clampedWidth + 'px';
    },

    endDragV: function () {
        this.isDraggingV = false;
        this.dividerV.classList.remove('dragging');
        document.body.classList.remove('resizing');
    },

    startDragH: function (clientY) {
        this.isDraggingH = true;
        this.startY = clientY;
        this.startHeight = this.attachments.offsetHeight;
        this.dividerH.classList.add('dragging');
        document.body.classList.add('resizing-h');
    },

    onDragH: function (clientY) {
        // Dragging up increases attachments height
        const delta = this.startY - clientY;
        const newHeight = this.startHeight + delta;
        const containerHeight = this.rightPane.offsetHeight;
        const maxHeight = containerHeight * this.MAX_ATTACHMENTS_RATIO;

        const clampedHeight = Math.max(this.MIN_ATTACHMENTS_HEIGHT, Math.min(maxHeight, newHeight));
        this.attachments.style.height = clampedHeight + 'px';
    },

    endDragH: function () {
        this.isDraggingH = false;
        this.dividerH.classList.remove('dragging');
        document.body.classList.remove('resizing-h');
    },

    clampSizes: function () {
        // Clamp left pane width on window resize
        const containerWidth = this.leftPane.parentElement.offsetWidth;
        const maxWidth = containerWidth * this.MAX_LEFT_RATIO;
        const currentWidth = this.leftPane.offsetWidth;

        if (currentWidth > maxWidth) {
            this.leftPane.style.width = maxWidth + 'px';
        } else if (currentWidth < this.MIN_LEFT_WIDTH) {
            this.leftPane.style.width = this.MIN_LEFT_WIDTH + 'px';
        }

        // Clamp attachments height on window resize
        const rightPaneHeight = this.rightPane.offsetHeight;
        const maxHeight = rightPaneHeight * this.MAX_ATTACHMENTS_RATIO;
        const currentHeight = this.attachments.offsetHeight;

        if (currentHeight > maxHeight) {
            this.attachments.style.height = maxHeight + 'px';
        } else if (currentHeight < this.MIN_ATTACHMENTS_HEIGHT) {
            this.attachments.style.height = this.MIN_ATTACHMENTS_HEIGHT + 'px';
        }
    }
};
