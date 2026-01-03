'use strict';

const Main = {
    dom: null,
    messageHandlers: null,
    paneElements: null,

    setActivePane: function (pane) {
        if (State.data.activePane === pane) return;

        State.data.activePane = pane;

        const panes = ['search', 'keyList', 'editor', 'attachments'];
        for (let i = 0; i < panes.length; i++) {
            const p = panes[i];
            const el = this.paneElements[p];
            if (el) {
                if (p === pane) {
                    el.classList.add('pane-active');
                    el.classList.remove('pane-inactive');
                } else {
                    el.classList.remove('pane-active');
                    el.classList.add('pane-inactive');
                }
            }
        }
    },

    selectOrCreateKey: function (query) {
        if (State.data.exactMatch !== 'none') {
            State.data.focusEditorOnLoad = true;
            KeyList.requestSelect(query);
        } else {
            State.data.pendingSelectKey = query;
            State.data.focusEditorOnLoad = true;
            Api.send({type: 'create', key: query});
        }
    },

    init: function () {
        const self = this;

        // Cache DOM references
        this.dom = {
            splash: document.getElementById('splash'),
            searchBar: document.querySelector('.search-bar'),
            searchInput: document.getElementById('search-input'),
            searchActionBtn: document.getElementById('search-action-btn'),
            leftPane: document.querySelector('.left-pane'),
            keyList: document.getElementById('key-list'),
            trashSection: document.getElementById('trash-section'),
            trashList: document.getElementById('trash-list'),
            trashCount: document.getElementById('trash-count'),
            editorContainer: document.getElementById('editor-container'),
            emptyState: document.getElementById('empty-state'),
            attachments: document.getElementById('attachments-container'),
        };

        // Map pane names to DOM elements for focus management
        this.paneElements = {
            search: this.dom.searchBar,
            keyList: this.dom.leftPane,
            editor: this.dom.editorContainer,
            attachments: this.dom.attachments,
        };

        // Initialize modules with DOM references
        KeyList.init({
            keyList: this.dom.keyList,
            trashSection: this.dom.trashSection,
            trashList: this.dom.trashList,
            trashCount: this.dom.trashCount,
        });

        Editor.init({
            editorContainer: this.dom.editorContainer,
            emptyState: this.dom.emptyState,
        }, function () {
            Api.send({type: 'ready'});
        });

        Attachments.init({
            container: this.dom.attachments,
        });

        // Set up event handlers
        this.setupEventHandlers();

        // Set up message handlers
        this.initMessageHandlers();
        this.setupMessageHandler();

        // Initial state
        this.initPaneClasses();
        this.dom.searchInput.focus();
        Editor.showEmpty();
    },

    initPaneClasses: function () {
        // Initialize all panes as inactive, then activate search
        const panes = ['search', 'keyList', 'editor', 'attachments'];
        for (let i = 0; i < panes.length; i++) {
            const el = this.paneElements[panes[i]];
            if (el) el.classList.add('pane-inactive');
        }
        this.setActivePane('search');
    },

    setupEventHandlers: function () {
        const self = this;

        // Search input focus
        this.dom.searchInput.addEventListener('focus', function () {
            self.setActivePane('search');
        });

        // Search input
        this.dom.searchInput.addEventListener('input', function (e) {
            if (e.target.value && State.data.selectedKey) {
                State.clearSelection();
                KeyList.updateSelection();
            }
            self.updateSearchButton();
            Api.send({type: 'search', query: e.target.value});
        });

        // Search action button
        this.dom.searchActionBtn.addEventListener('click', function () {
            const query = self.dom.searchInput.value.trim();
            if (!query) return;

            self.selectOrCreateKey(query);
            self.dom.searchInput.value = '';
            self.updateSearchButton();
        });

        // Search enter key and arrow navigation
        this.dom.searchInput.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                const query = self.dom.searchInput.value.trim();
                if (query) {
                    self.selectOrCreateKey(query);
                }
            } else if (e.key === 'ArrowDown') {
                e.preventDefault();
                const firstItem = document.querySelector('.key-item');
                if (firstItem) {
                    const keyName = firstItem.dataset.key;
                    if (keyName) KeyList.requestSelect(keyName);
                    self.setActivePane('keyList');
                    firstItem.focus();
                }
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
            }
        });

        // Key list focus (clicking on any key item)
        this.dom.keyList.addEventListener('focusin', function (e) {
            if (e.target.classList.contains('key-item')) {
                self.setActivePane('keyList');
            }
        });

        // Trash list focus (clicking on any trash item)
        this.dom.trashList.addEventListener('focusin', function (e) {
            if (e.target.classList.contains('trash-item')) {
                self.setActivePane('keyList');
            }
        });

        // Key list keyboard navigation
        this.dom.keyList.addEventListener('keydown', function (e) {
            const focused = document.activeElement;
            if (!focused || !focused.classList.contains('key-item')) return;

            if (e.key === 'Enter') {
                e.preventDefault();
                if (Editor.instance) {
                    self.setActivePane('editor');
                    Editor.instance.focus();
                }
            } else if (e.key === 'ArrowDown') {
                e.preventDefault();
                const next = focused.nextElementSibling;
                if (next && next.classList.contains('key-item')) {
                    const keyName = next.dataset.key;
                    if (keyName) KeyList.requestSelect(keyName);
                    next.focus();
                }
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                const prev = focused.previousElementSibling;
                if (prev && prev.classList.contains('key-item')) {
                    const keyName = prev.dataset.key;
                    if (keyName) KeyList.requestSelect(keyName);
                    prev.focus();
                } else {
                    self.setActivePane('search');
                    self.dom.searchInput.focus();
                }
            }
        });

        // Trash list keyboard navigation
        this.dom.trashList.addEventListener('keydown', function (e) {
            const focused = document.activeElement;
            if (!focused || !focused.classList.contains('trash-item')) return;

            if (e.key === 'ArrowDown') {
                e.preventDefault();
                const next = focused.nextElementSibling;
                if (next && next.classList.contains('trash-item')) {
                    const keyName = next.dataset.key;
                    if (keyName) KeyList.requestSelect(keyName);
                    next.focus();
                }
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                const prev = focused.previousElementSibling;
                if (prev && prev.classList.contains('trash-item')) {
                    const keyName = prev.dataset.key;
                    if (keyName) KeyList.requestSelect(keyName);
                    prev.focus();
                }
            }
        });

        // Global keyboard shortcuts
        document.addEventListener('keydown', function (e) {
            if (e.key === 'Tab') {
                e.preventDefault();
            } else if (e.key === 'Escape') {
                Api.send({type: 'hide'});
            } else if (e.key === 's' && e.ctrlKey && !e.altKey && !e.shiftKey) {
                e.preventDefault();
                self.setActivePane('search');
                self.dom.searchInput.focus();
                self.dom.searchInput.select();
            }
        });
    },

    initMessageHandlers: function () {
        const self = this;

        this.messageHandlers = {
            coreReady: function () {
                self.hideSplash();
            },

            theme: function (msg) {
                Editor.applyTheme(msg.theme);
            },

            searchResults: function (msg) {
                State.data.keys = msg.activeKeys;
                State.data.trashedKeys = msg.trashedKeys;
                State.data.exactMatch = msg.exactMatch;

                if (State.data.selectedKey && !State.isKeyVisible(State.data.selectedKey)) {
                    State.clearSelection();
                    Editor.showEmpty();
                }

                KeyList.render();
                KeyList.renderTrash();
                self.updateSearchButton();

                if (State.data.pendingSelectKey) {
                    // Only process if key exists in results (handles race with async search)
                    if (State.data.keys.indexOf(State.data.pendingSelectKey) !== -1) {
                        KeyList.requestSelect(State.data.pendingSelectKey);
                        const item = document.querySelector('.key-item[data-key="' + CSS.escape(State.data.pendingSelectKey) + '"]');
                        if (item) item.focus();
                        State.data.pendingSelectKey = null;
                    }
                }
            },

            value: async function (msg, event) {
                if (msg.key !== State.data.selectedKey) return;

                // Check for FileSystemHandle in additionalObjects
                if (event.additionalObjects && event.additionalObjects.length > 0) {
                    const handle = event.additionalObjects[0];
                    await Editor.showWithHandle(handle, msg.key, msg.readOnly);
                } else {
                    // No handle received - show error
                    console.error('[Main] No FileSystemHandle in message');
                    Editor.showError('Failed to access content');
                }
            },

            keyCreated: function (msg) {
                if (!msg.success) {
                    alert('Key already exists: ' + msg.key);
                    State.data.pendingSelectKey = null;
                }
            },

            renameResult: function (msg) {
                KeyList.handleRenameResult(msg.oldKey, msg.newKey, msg.result);
            },

            shutdown: async function () {
                if (State.data.isShuttingDown) {
                    Api.send({type: 'shutdownAck'});
                    return;
                }
                State.data.isShuttingDown = true;
                self.showShutdownOverlay();
                await Editor.forceSave();
                Api.send({type: 'shutdownAck'});
            },

            focus: function () {
                const pane = State.data.activePane;

                if (pane === 'search') {
                    self.dom.searchInput.focus();
                    self.dom.searchInput.select();
                } else if (pane === 'keyList') {
                    const selected = document.querySelector('.key-item.selected, .trash-item.selected');
                    if (selected) selected.focus();
                } else if (pane === 'editor') {
                    if (Editor.instance) Editor.instance.focus();
                } else if (pane === 'attachments') {
                    const item = document.querySelector('.attachment-item.selected') ||
                                 document.querySelector('.attachment-item');
                    if (item) item.focus();
                }
            }
        };
    },

    setupMessageHandler: function () {
        const self = this;

        window.chrome.webview.addEventListener('message', async function (event) {
            const msg = event.data;
            const handler = self.messageHandlers[msg.type];
            if (handler) {
                await handler(msg, event);
            }
        });
    },

    updateSearchButton: function () {
        const query = this.dom.searchInput.value.trim();

        if (!query || State.data.selectedKey) {
            this.dom.searchActionBtn.classList.remove('visible');
            return;
        }

        if (State.data.exactMatch !== 'none') {
            this.dom.searchActionBtn.innerHTML = '&#9998;';
            this.dom.searchActionBtn.title = 'Edit key';
        } else {
            this.dom.searchActionBtn.innerHTML = '&#43;';
            this.dom.searchActionBtn.title = 'Create key';
        }
        this.dom.searchActionBtn.classList.add('visible');
    },

    hideSplash: function () {
        if (this.dom.splash && !this.dom.splash.classList.contains('hidden')) {
            this.dom.splash.classList.add('hidden');
            const splash = this.dom.splash;
            setTimeout(function () {
                splash.remove();
            }, 150);
        }
    },

    showShutdownOverlay: function () {
        const overlay = document.createElement('div');
        overlay.style.cssText = 'position:fixed;top:0;left:0;right:0;bottom:0;background:rgba(0,0,0,0.3);z-index:9999;';
        document.body.appendChild(overlay);
    }
};

// Entry point
Main.init();
