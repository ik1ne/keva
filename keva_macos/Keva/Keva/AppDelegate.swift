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

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let webVC = window?.webViewController else {
            return .terminateNow
        }

        // Ask frontend to save before shutting down
        webVC.initiateShutdown()
        return .terminateLater
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        window?.show()
        return true
    }
}
