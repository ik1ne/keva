'use strict';

const Main = {
    dom: null,

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

        // Set up message handler
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

            if (State.data.exactMatch !== 'none') {
                KeyList.requestSelect(query);
            } else {
                Api.send({type: 'create', key: query});
            }
            self.dom.searchInput.value = '';
            self.updateSearchButton();
        });

        // Search enter key and arrow navigation
        this.dom.searchInput.addEventListener('keydown', function (e) {
            if (e.key === 'Enter') {
                const query = self.dom.searchInput.value.trim();
                if (query) {
                    if (State.data.exactMatch !== 'none') {
                        KeyList.requestSelect(query);
                    } else {
                        State.data.pendingSelectKey = query;
                        Api.send({type: 'create', key: query});
                    }
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

    setupMessageHandler: function () {
        const self = this;

        window.chrome.webview.addEventListener('message', function (event) {
            const msg = event.data;
            switch (msg.type) {
                case 'coreReady':
                    self.hideSplash();
                    break;

                case 'theme':
                    Editor.applyTheme(msg.theme);
                    break;

                case 'searchResults':
                    State.data.keys = msg.keys.filter(function (k) {
                        return !k.trashed;
                    });
                    State.data.trashedKeys = msg.keys.filter(function (k) {
                        return k.trashed;
                    });
                    State.data.exactMatch = msg.exactMatch;

                    if (State.data.selectedKey && !State.isKeyVisible(State.data.selectedKey)) {
                        State.clearSelection();
                        Editor.showEmpty();
                    }

                    KeyList.render();
                    KeyList.renderTrash();
                    self.updateSearchButton();

                    if (State.data.pendingSelectKey) {
                        KeyList.requestSelect(State.data.pendingSelectKey);
                        State.data.pendingSelectKey = null;
                    }
                    break;

                case 'value':
                    if (msg.key !== State.data.selectedKey) break;
                    if (msg.value) {
                        if (msg.value.type === 'text') {
                            Editor.show(msg.value.content);
                        } else if (msg.value.type === 'files') {
                            Editor.showEmpty(msg.value.count + ' file(s)');
                        }
                    } else {
                        Editor.showEmpty('Key not found');
                    }
                    break;

                case 'keyCreated':
                    if (!msg.success) {
                        alert('Key already exists: ' + msg.key);
                        State.data.pendingSelectKey = null;
                    }
                    break;

                case 'shutdown':
                    if (State.data.isShuttingDown) {
                        Api.send({type: 'shutdownAck'});
                        break;
                    }
                    State.data.isShuttingDown = true;
                    self.showShutdownOverlay();
                    Editor.forceSave();
                    Api.send({type: 'shutdownAck'});
                    break;

                case 'focus':
                    self.dom.searchInput.focus();
                    break;
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
