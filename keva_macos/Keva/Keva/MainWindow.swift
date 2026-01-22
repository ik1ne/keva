import Cocoa

/// Borderless floating window that hosts the Keva WebView.
class MainWindow: NSWindow, NSWindowDelegate {
    private(set) var webViewController: WebViewController!
    private var eventMonitor: Any?
    private var previousApp: NSRunningApplication?

    init() {
        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.borderless, .resizable],
            backing: .buffered,
            defer: false
        )

        delegate = self
        level = .floating
        minSize = NSSize(width: 400, height: 300)
        isMovableByWindowBackground = true
        center()

        webViewController = WebViewController()
        contentViewController = webViewController

        setupKeyEventMonitor()
        setupWorkspaceNotifications()
    }

    deinit {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
        }
        NSWorkspace.shared.notificationCenter.removeObserver(self)
    }

    // MARK: - NSWindow Overrides

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    // MARK: - NSWindowDelegate

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        hide()
        return false
    }

    // MARK: - Window Visibility

    func show() {
        makeKeyAndOrderFront(nil)
        NSApp.activate()
    }

    func hide() {
        orderOut(nil)
        restorePreviousApp()
    }

    func toggle() {
        if isVisible {
            hide()
        } else {
            show()
        }
    }

    // MARK: - Focus Restore

    private func restorePreviousApp() {
        guard let app = previousApp, !app.isTerminated else { return }
        app.activate()
    }

    private func setupWorkspaceNotifications() {
        NSWorkspace.shared.notificationCenter.addObserver(
            self,
            selector: #selector(applicationDidActivate(_:)),
            name: NSWorkspace.didActivateApplicationNotification,
            object: nil
        )
    }

    @objc private func applicationDidActivate(_ notification: Notification) {
        guard let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication,
              app.bundleIdentifier != Bundle.main.bundleIdentifier else {
            return
        }
        previousApp = app
    }

    // MARK: - Keyboard Handling

    private func setupKeyEventMonitor() {
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self = self else { return event }

            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

            // Cmd+Q: Quit application
            if modifiers == .command && event.characters?.lowercased() == "q" {
                NSApp.terminate(nil)
                return nil
            }

            // Esc: Hide window
            // TODO: M5c will move this to frontend (context-aware via 'hide' message)
            if event.keyCode == 53 {
                self.hide()
                return nil
            }

            return event
        }
    }
}
