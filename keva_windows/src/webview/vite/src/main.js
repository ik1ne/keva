'use strict';

import './styles.css';

import {State, setEditorRef} from './state.js';
import {Api} from './api.js';
import {Editor, setMainRef as setEditorMainRef} from './editor.js';
import {KeyList, setMainRef as setKeyListMainRef} from './keylist.js';
import {Attachments, setMainRef as setAttachmentsMainRef} from './attachments.js';
import {Drop, setMainRef as setDropMainRef} from './drop.js';
import {ConflictDialog} from './conflict-dialog.js';
import {Settings} from './settings.js';
import {Welcome} from './welcome.js';
import {showToast} from './toast.js';
import {Resizer} from './resizer.js';
import {Dialog} from './dialog.js';
import {SaveBanner} from './save-banner.js';

export const Main = {
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
            if (query === State.data.selectedKey) {
                // Already selected, just focus editor
                this.setActivePane('editor');
                if (Editor.instance) Editor.instance.focus();
            } else {
                State.data.focusEditorOnLoad = true;
                KeyList.requestSelect(query);
            }
        } else {
            State.data.pendingSelectKey = query;
            State.data.focusEditorOnLoad = true;
            Api.send({type: 'create', key: query});
        }
    },

    init: function () {
        const self = this;

        // Wire up circular dependencies
        setEditorRef(Editor);
        setEditorMainRef(this);
        setKeyListMainRef(this);
        setAttachmentsMainRef(this);
        setDropMainRef(this);

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
            editorTabs: document.getElementById('editor-tabs'),
            editorViewport: document.querySelector('.editor-viewport'),
            editorContainer: document.getElementById('editor-container'),
            previewContainer: document.getElementById('preview-container'),
            emptyState: document.getElementById('empty-state'),
            attachments: document.getElementById('attachments-container'),
            addFilesBtn: document.getElementById('add-files-btn'),
            dividerV: document.getElementById('divider-v'),
            dividerH: document.getElementById('divider-h'),
            rightPane: document.querySelector('.right-pane'),
        };

        // Map pane names to DOM elements for focus management
        this.paneElements = {
            search: this.dom.searchBar,
            keyList: this.dom.leftPane,
            editor: this.dom.editorViewport,
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
            editorTabs: this.dom.editorTabs,
            editorViewport: this.dom.editorViewport,
            previewContainer: this.dom.previewContainer,
        }, function () {
            Api.send({type: 'ready'});
        });

        SaveBanner.init(this.dom.editorViewport);

        // Set up tab handlers
        this.setupTabHandlers();

        Attachments.init({
            container: this.dom.attachments,
        });

        Drop.init();

        Settings.init();

        Welcome.init();

        Resizer.init({
            leftPane: this.dom.leftPane,
            dividerV: this.dom.dividerV,
            dividerH: this.dom.dividerH,
            attachments: this.dom.attachments,
            editorViewport: this.dom.editorViewport,
            rightPane: this.dom.rightPane,
        });

        // Set up event handlers
        this.setupEventHandlers();

        // Set up message handlers
        this.initMessageHandlers();
        this.setupMessageHandler();

        // Initial state
        this.initPaneClasses();
        this.updateAddFilesBtn();
        this.dom.searchInput.focus();
        Editor.resetState();
        this.hideEditorUI();
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
                State.data.attachments = [];
                Attachments.render();
                KeyList.updateSelection();
                Editor.resetState();
                self.hideEditorUI();
                self.updateAddFilesBtn();
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
                e.preventDefault();
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

        // Add files button
        this.dom.addFilesBtn.addEventListener('click', function () {
            if (!State.data.selectedKey || State.data.isSelectedTrashed) return;
            Api.send({type: 'openFilePicker', key: State.data.selectedKey});
        });

        // Global keyboard shortcuts
        document.addEventListener('keydown', function (e) {
            if (e.key === 'Tab') {
                e.preventDefault();
            } else if (e.key === 'Escape') {
                // During drag, just cancel the drag instead of hiding
                if (Drop.isDragging) {
                    Drop.isDragging = false;
                    Drop.clearDropCursor();
                    return;
                }
                // Don't hide if conflict dialog is open (it handles Escape itself)
                if (ConflictDialog.overlay) {
                    return;
                }
                // Don't hide if settings panel handled this Escape
                if (Settings.handledEscape) {
                    return;
                }
                Api.send({type: 'hide'});
            }
        });

        // Copy event handler for attachments pane
        document.addEventListener('copy', function (e) {
            if (State.data.activePane === 'attachments' && Attachments.selectedIndices.size > 0) {
                e.preventDefault();
                const selectedItems = Attachments.getSelectedItems();
                const filenames = selectedItems.map(function (item) {
                    return item.dataset.filename;
                });
                if (filenames.length > 0 && State.data.selectedKey) {
                    Api.send({
                        type: 'copyFiles',
                        key: State.data.selectedKey,
                        filenames: filenames
                    });
                }
            }
            // Otherwise, let browser handle copy (Monaco, search, etc.)
        });
    },

    setupTabHandlers: function () {
        const self = this;
        const tabs = this.dom.editorTabs.querySelectorAll('.tab');

        tabs.forEach(function (tab) {
            tab.addEventListener('click', function () {
                const mode = tab.dataset.tab;
                if (State.data.editorMode === mode) return;
                self.switchEditorMode(mode);
            });
        });
    },

    switchEditorMode: function (mode) {
        const editorContainer = this.dom.editorContainer;
        const previewContainer = this.dom.previewContainer;
        const tabs = this.dom.editorTabs.querySelectorAll('.tab');

        // Set editor pane as active when switching modes
        this.setActivePane('editor');

        // Save scroll position before switch
        if (State.data.editorMode === 'preview') {
            Editor.previewScrollTop = previewContainer.scrollTop;
        } else if (Editor.instance) {
            Editor.editScrollTop = Editor.instance.getScrollTop();
        }

        State.data.editorMode = mode;

        // Update tab active state
        tabs.forEach(function (tab) {
            if (tab.dataset.tab === mode) {
                tab.classList.add('active');
            } else {
                tab.classList.remove('active');
            }
        });

        if (mode === 'preview') {
            editorContainer.style.display = 'none';
            previewContainer.classList.remove('hidden');
            previewContainer.innerHTML = Editor.renderPreview();
            this.setupPreviewImages(previewContainer);
            previewContainer.scrollTop = Editor.previewScrollTop || 0;
        } else {
            previewContainer.classList.add('hidden');
            editorContainer.style.display = 'block';
            if (Editor.instance) {
                Editor.instance.focus();
                if (Editor.editScrollTop != null) {
                    Editor.instance.setScrollTop(Editor.editScrollTop);
                }
            }
        }
    },

    setupPreviewImages: function (container) {
        container.querySelectorAll('img').forEach(function (img) {
            img.addEventListener('error', function () {
                img.classList.add('broken-image');
                img.title = 'Attachment not found: ' + (img.alt || 'unknown');
            });
        });
    },

    showEditorUI: function () {
        this.dom.editorTabs.style.display = 'flex';
        this.dom.editorViewport.style.display = 'block';
        this.dom.emptyState.style.display = 'none';
        this.dom.dividerH.style.display = 'block';
        this.dom.attachments.style.display = 'flex';
    },

    hideEditorUI: function (message) {
        this.dom.editorTabs.style.display = 'none';
        this.dom.editorViewport.style.display = 'none';
        this.dom.previewContainer.classList.add('hidden');
        this.dom.editorContainer.style.display = 'none';
        this.dom.emptyState.textContent = message || 'Select a key or type to search';
        this.dom.emptyState.style.display = 'flex';
        this.dom.dividerH.style.display = 'none';
        this.dom.attachments.style.display = 'none';
    },

    resetToEditMode: function () {
        State.data.editorMode = 'edit';
        Editor.editScrollTop = 0;
        Editor.previewScrollTop = 0;
        Editor.invalidatePreviewCache();

        const tabs = this.dom.editorTabs.querySelectorAll('.tab');
        tabs.forEach(function (tab) {
            if (tab.dataset.tab === 'edit') {
                tab.classList.add('active');
            } else {
                tab.classList.remove('active');
            }
        });

        this.dom.previewContainer.classList.add('hidden');
        this.dom.editorContainer.style.display = 'block';
    },

    initMessageHandlers: function () {
        const self = this;

        this.messageHandlers = {
            coreReady: function () {
                self.hideSplash();
                // Focus search bar on first launch
                self.dom.searchInput.focus();
            },

            theme: function (msg) {
                Editor.applyTheme(msg.theme);
            },

            searchResults: function (msg) {
                State.data.keys = msg.activeKeys;
                State.data.trashedKeys = msg.trashedKeys;
                State.data.exactMatch = msg.exactMatch;

                var trashStateChanged = false;
                // Check if selected key is exact match (O(1) lookup, valid even if async search hasn't found it)
                var searchQuery = self.dom.searchInput.value.trim();
                var isSelectedExactMatch = State.data.selectedKey === searchQuery && msg.exactMatch !== 'none';
                if (State.data.selectedKey && !State.isKeyVisible(State.data.selectedKey) && !isSelectedExactMatch) {
                    State.clearSelection();
                    State.data.attachments = [];
                    Attachments.render();
                    Editor.resetState();
                    self.hideEditorUI();
                    self.updateAddFilesBtn();
                } else if (State.data.selectedKey) {
                    // Update readonly state if key moved between active and trashed.
                    // Use exactMatch for O(1) lookup when key matches query, fall back to results array.
                    var isTrashed = isSelectedExactMatch
                        ? msg.exactMatch === 'trashed'
                        : msg.trashedKeys.indexOf(State.data.selectedKey) !== -1;
                    trashStateChanged = State.data.isSelectedTrashed !== isTrashed;
                    if (trashStateChanged) {
                        State.data.isSelectedTrashed = isTrashed;
                        Editor.isReadOnly = isTrashed;
                        if (Editor.instance) {
                            Editor.instance.updateOptions({readOnly: isTrashed});
                        }
                        Editor.updatePlaceholder();
                        Attachments.render();
                        self.updateAddFilesBtn();
                    }
                }

                KeyList.render();
                KeyList.renderTrash();
                self.updateSearchButton();

                // Focus the selected key in its new list after trash/restore
                if (State.data.selectedKey && trashStateChanged) {
                    const selector = State.data.isSelectedTrashed ? '.trash-item' : '.key-item';
                    const item = document.querySelector(selector + '[data-key="' + CSS.escape(State.data.selectedKey) + '"]');
                    if (item) item.focus();
                }

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

                // File copy completed
                State.data.isCopying = false;

                // Hide adding overlay if shown
                self.hideAddingOverlay();

                // Update attachments
                State.data.attachments = msg.attachments || [];
                Attachments.render();
                self.updateAddFilesBtn();

                // Store keyHash for preview rendering
                Editor.keyHash = msg.keyHash;

                // Insert links if pending from drop/paste
                if (State.data.pendingLinkInsert) {
                    Drop.insertAttachmentLinks(
                        State.data.pendingLinkInsert.files,
                        State.data.pendingLinkInsert.selection
                    );
                    State.data.pendingLinkInsert = null;
                }

                // Check for pending copy action (from Ctrl+Alt+T/R/F with exact match)
                const pendingCopyAction = State.data.pendingCopyAction;
                State.data.pendingCopyAction = null;

                // Skip editor reload if already showing this key (e.g., after adding attachments)
                if (Editor.currentKey === msg.key) {
                    // Invalidate preview cache since attachments may have changed
                    Editor.invalidatePreviewCache();
                    // Handle pending copy (files action can proceed, content already loaded)
                    if (pendingCopyAction) {
                        self.performCopy(pendingCopyAction);
                    }
                    return;
                }

                // Reset to edit mode when switching keys
                self.resetToEditMode();

                // Check for FileSystemHandle in additionalObjects
                if (event.additionalObjects && event.additionalObjects.length > 0) {
                    const handle = event.additionalObjects[0];
                    self.showEditorUI();
                    await Editor.showWithHandle(handle, msg.key, msg.readOnly);
                    // Handle pending copy after editor is loaded
                    if (pendingCopyAction) {
                        self.performCopy(pendingCopyAction);
                    }
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

                if (State.data.isCopying) {
                    Api.send({type: 'shutdownBlocked'});
                    return;
                }
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
            },

            focusSearch: function () {
                self.setActivePane('search');
                self.dom.searchInput.focus();
                self.dom.searchInput.select();
            },

            filesSelected: function (msg) {
                // Build file list (id = path for file picker)
                const fileList = [];
                for (let i = 0; i < msg.files.length; i++) {
                    const path = msg.files[i];
                    fileList.push({id: path, filename: path.split(/[/\\]/).pop()});
                }
                const result = ConflictDialog.checkConflicts(fileList);

                if (result.conflicts.length > 0) {
                    ConflictDialog.show(msg.key, result.conflicts, result.nonConflicts);
                } else {
                    State.data.isCopying = true;
                    self.showAddingOverlay();
                    Api.send({type: 'addAttachments', key: msg.key, files: result.nonConflicts});
                }
            },

            filesPasted: function (msg) {
                // Reject if search bar focused or no key selected
                if (State.data.activePane === 'search') return;
                if (!State.data.selectedKey) return;
                if (State.data.isSelectedTrashed) {
                    showToast('Cannot add files to trashed key');
                    return;
                }

                // Determine if we should insert links (editor focused)
                const insertLinks = (State.data.activePane === 'editor');
                // Use selection to replace selected text when pasting
                const editorSelection = insertLinks && Editor.instance
                    ? Editor.instance.getSelection()
                    : null;

                // Build file list and check for conflicts
                const fileList = [];
                for (let i = 0; i < msg.files.length; i++) {
                    fileList.push({id: i, filename: msg.files[i]});
                }
                const result = ConflictDialog.checkConflicts(fileList);

                if (result.conflicts.length > 0) {
                    ConflictDialog.show(State.data.selectedKey, result.conflicts, result.nonConflicts, insertLinks, editorSelection, self.sendFiles.bind(self));
                } else {
                    self.sendFiles(State.data.selectedKey, result.nonConflicts, insertLinks, editorSelection);
                }
            },

            doCopy: function (msg) {
                const action = msg.action;

                if (action === 'markdown' || action === 'html') {
                    // Check if content is ready
                    if (State.data.selectedKey && Editor.instance && Editor.instance.getValue()) {
                        self.performCopy(action);
                    } else if (State.data.exactMatch === 'active') {
                        // Exact match but content not loaded yet, trigger load and wait
                        State.data.pendingCopyAction = action;
                        const key = self.dom.searchInput.value.trim();
                        State.data.selectedKey = key;
                        Api.send({type: 'select', key: key});
                    } else {
                        showToast('Nothing to copy');
                    }
                } else if (action === 'files') {
                    // Check if we have attachments to copy
                    if (State.data.selectedKey && State.data.attachments.length > 0) {
                        const filenames = State.data.attachments.map(function (a) {
                            return a.filename;
                        });
                        Api.send({
                            type: 'copyFiles',
                            key: State.data.selectedKey,
                            filenames: filenames
                        });
                    } else if (State.data.exactMatch === 'active') {
                        // Exact match but content not loaded yet, trigger load and wait
                        State.data.pendingCopyAction = action;
                        const key = self.dom.searchInput.value.trim();
                        State.data.selectedKey = key;
                        Api.send({type: 'select', key: key});
                    } else {
                        showToast('No attachments to copy');
                    }
                }
            },

            copyResult: function (msg) {
                if (msg.success) {
                    Api.send({type: 'hide'});
                } else {
                    showToast('Failed to copy files');
                }
            },

            openSettings: function (msg) {
                // If settings already open, ignore (prevents resetting unsaved changes/tab)
                // If capturing hotkey, show toast about Ctrl+Comma being reserved
                if (!Settings.overlay.classList.contains('hidden')) {
                    if (Settings.isCapturingHotkey) {
                        showToast('Ctrl+, is reserved for Open Settings');
                    }
                    return;
                }
                Settings.open(msg.config, msg.launchAtLogin);
            },

            toast: function (msg) {
                showToast(msg.message);
            },

            saveFailed: function (msg) {
                SaveBanner.show(msg.message, Editor.retrySave.bind(Editor));
            },

            coreInitFailed: function (msg) {
                var message = (msg.message || 'Core initialization failed.');
                if (msg.dataDir) {
                    message += '\n\nData directory:\n' + msg.dataDir;
                }
                Dialog.show({
                    message: message,
                    buttons: [{label: 'Quit', action: 'quit', danger: true, primary: true, focus: true}],
                    onClose: function () {
                        Api.send({type: 'shutdownAck'});
                    },
                    onEscape: function () {
                        Api.send({type: 'shutdownAck'});
                    }
                });
            },

            showWelcome: function () {
                Welcome.show();
            }
        };
    },

    sendFiles: function (key, files, insertLinks, editorPosition) {
        if (files.length === 0) return;

        State.data.isCopying = true;
        this.showAddingOverlay();

        // Store pending link insertion info
        if (insertLinks && editorPosition) {
            State.data.pendingLinkInsert = {
                files: files.map(function (f) {
                    return f[1];
                }), // filenames
                selection: editorPosition // insertAttachmentLinks handles both position and selection
            };
        }

        Api.send({
            type: 'addFiles',
            key: key,
            files: files // [[index, filename], ...]
        });
    },

    performCopy: function (action) {
        if (action === 'markdown') {
            const text = Editor.instance ? Editor.instance.getValue() : '';
            if (!text) {
                showToast('Nothing to copy');
                return;
            }
            navigator.clipboard.writeText(text).then(function () {
                Api.send({type: 'hide'});
            }).catch(function () {
                showToast('Failed to copy');
            });
        } else if (action === 'html') {
            const html = Editor.instance ? Editor.renderPreviewForExport() : '';
            if (!html) {
                showToast('Nothing to copy');
                return;
            }
            navigator.clipboard.writeText(html).then(function () {
                Api.send({type: 'hide'});
            }).catch(function () {
                showToast('Failed to copy');
            });
        } else if (action === 'files') {
            if (State.data.attachments.length === 0) {
                showToast('No attachments to copy');
                return;
            }
            const filenames = State.data.attachments.map(function (a) {
                return a.filename;
            });
            Api.send({
                type: 'copyFiles',
                key: State.data.selectedKey,
                filenames: filenames
            });
        }
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

    updateAddFilesBtn: function () {
        const canAdd = State.data.selectedKey && !State.data.isSelectedTrashed;
        this.dom.addFilesBtn.disabled = !canAdd;
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
        overlay.className = 'blocking-overlay';
        document.body.appendChild(overlay);
    },

    addingOverlay: null,
    addingOverlayTimer: null,

    showAddingOverlay: function () {
        if (this.addingOverlay || this.addingOverlayTimer) return;
        // Delay showing overlay to avoid brief blink for small/fast copies
        this.addingOverlayTimer = setTimeout(() => {
            this.addingOverlayTimer = null;
            this.addingOverlay = document.createElement('div');
            this.addingOverlay.className = 'blocking-overlay';
            this.addingOverlay.innerHTML = '<div class="blocking-overlay-message">Adding files...</div>';
            document.body.appendChild(this.addingOverlay);
        }, 150);
    },

    hideAddingOverlay: function () {
        if (this.addingOverlayTimer) {
            clearTimeout(this.addingOverlayTimer);
            this.addingOverlayTimer = null;
        }
        if (this.addingOverlay) {
            this.addingOverlay.remove();
            this.addingOverlay = null;
        }
    }
};

// Entry point
Main.init();
