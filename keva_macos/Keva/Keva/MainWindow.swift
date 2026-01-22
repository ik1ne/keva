import Cocoa

class MainWindow: NSWindow, NSWindowDelegate {
    private var eventMonitor: Any?

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
    }

    deinit {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        orderOut(nil)
        return false
    }

    func show() {
        makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func hide() {
        orderOut(nil)
    }

    func toggle() {
        if isVisible {
            hide()
        } else {
            show()
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
