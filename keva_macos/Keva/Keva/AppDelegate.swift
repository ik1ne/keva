import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: MainWindow?
    private var statusItemController: StatusItemController?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSLog("Keva launched")

        window = MainWindow()
        window?.show()

        statusItemController = StatusItemController(window: window!)
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        window?.show()
        return true
    }
}
