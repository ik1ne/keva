import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    var window: NSWindow?
    var eventMonitor: Any?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSLog("Keva launched")

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.borderless, .resizable],
            backing: .buffered,
            defer: false
        )
        window?.delegate = self
        window?.level = .floating
        window?.minSize = NSSize(width: 400, height: 300)
        window?.isMovableByWindowBackground = true
        window?.center()
        window?.makeKeyAndOrderFront(nil)

        setupKeyEventMonitor()
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let monitor = eventMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showWindow()
        return true
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        sender.orderOut(nil)
        return false
    }

    func showWindow() {
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func hideWindow() {
        window?.orderOut(nil)
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
                self.hideWindow()
                return nil
            }

            return event
        }
    }
}
