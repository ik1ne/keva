'use strict';

const Main = {
    dom: null,
    messageHandlers: null,

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
            searchInput: document.getElementById('search-input'),
            searchActionBtn: document.getElementById('search-action-btn'),
            keyList: document.getElementById('key-list'),
            trashSection: document.getElementById('trash-section'),
            trashList: document.getElementById('trash-list'),
            trashCount: document.getElementById('trash-count'),
            editorContainer: document.getElementById('editor-container'),
            emptyState: document.getElementById('empty-state'),
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

        // Set up event handlers
        this.setupEventHandlers();

        // Set up message handlers
        this.initMessageHandlers();
        this.setupMessageHandler();

        // Initial state
        this.dom.searchInput.focus();
        Editor.showEmpty();
    },

    setupEventHandlers: function () {
        const self = this;

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
                    firstItem.focus();
                }
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
            }
        });

        // Key list keyboard navigation
        this.dom.keyList.addEventListener('keydown', function (e) {
            const focused = document.activeElement;
            if (!focused || !focused.classList.contains('key-item')) return;

            if (e.key === 'Enter') {
                e.preventDefault();
                if (Editor.instance) {
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
                    self.dom.searchInput.focus();
                }
            }
        });

        // Global escape handler
        document.addEventListener('keydown', function (e) {
            if (e.key === 'Escape') {
                Api.send({type: 'hide'});
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

            value: function (msg) {
                if (msg.key !== State.data.selectedKey) return;
                if (msg.value) {
                    if (msg.value.type === 'text') {
                        Editor.show(msg.value.content);
                        if (State.data.focusEditorOnLoad && Editor.instance) {
                            Editor.instance.focus();
                        }
                    } else if (msg.value.type === 'files') {
                        Editor.showEmpty(msg.value.count + ' file(s)');
                    }
                } else {
                    Editor.showEmpty('Key not found');
                }
                State.data.focusEditorOnLoad = false;
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

            shutdown: function () {
                if (State.data.isShuttingDown) {
                    Api.send({type: 'shutdownAck'});
                    return;
                }
                State.data.isShuttingDown = true;
                self.showShutdownOverlay();
                Editor.forceSave();
                Api.send({type: 'shutdownAck'});
            },

            focus: function () {
                self.dom.searchInput.focus();
            }
        };
    },

    setupMessageHandler: function () {
        const self = this;

        window.chrome.webview.addEventListener('message', function (event) {
            const msg = event.data;
            const handler = self.messageHandlers[msg.type];
            if (handler) {
                handler(msg);
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
