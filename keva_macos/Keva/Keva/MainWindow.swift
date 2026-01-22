import Cocoa

class MainWindow: NSWindow, NSWindowDelegate {
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

        setupKeyEventMonitor()
        setupWorkspaceNotifications()
    }

    deinit {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
        }
        NSWorkspace.shared.notificationCenter.removeObserver(self)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        hide()
        return false
    }

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

    /// Track app activations to capture the previous app for focus restore.
    @objc private func applicationDidActivate(_ notification: Notification) {
        guard let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication else {
            return
        }
        // Store the activated app if it's not ourselves
        if app.bundleIdentifier != Bundle.main.bundleIdentifier {
            previousApp = app
        }
    }

    private func setupKeyEventMonitor() {
        eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            guard let self = self else { return event }

            let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

            // Cmd+Q: Quit application
            // Use `characters` (not `charactersIgnoringModifiers`) because macOS switches to
            // Latin layer when Command is pressed, even on non-Latin keyboards (Korean, Hebrew, etc.)
            if modifiers == .command && event.characters?.lowercased() == "q" {
                NSApp.terminate(nil)
                return nil
            }

            // Esc: Hide window
            if event.keyCode == 53 {
                self.hide()
                return nil
            }

            return event
        }
    }
}
