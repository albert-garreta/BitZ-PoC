// Offscreen WebKit snapshot of a local HTML page.
// usage: swift snap.swift <page.html> <out.png> <width> [light|dark]
import Cocoa
import WebKit

let args = CommandLine.arguments
let input = URL(fileURLWithPath: args[1])
let output = URL(fileURLWithPath: args[2])
let width = CGFloat(Double(args[3]) ?? 1280)
let dark = args.count > 4 && args[4] == "dark"

final class Loader: NSObject, WKNavigationDelegate {
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 2.0) {
            webView.evaluateJavaScript("document.documentElement.scrollHeight") { result, _ in
                let height = CGFloat((result as? NSNumber)?.doubleValue ?? 2000)
                webView.frame = NSRect(x: 0, y: 0, width: width, height: height)
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                    let config = WKSnapshotConfiguration()
                    config.rect = NSRect(x: 0, y: 0, width: width, height: height)
                    webView.takeSnapshot(with: config) { image, error in
                        guard let image, let tiff = image.tiffRepresentation,
                              let bitmap = NSBitmapImageRep(data: tiff),
                              let png = bitmap.representation(using: .png, properties: [:]) else {
                            print("snapshot failed: \(String(describing: error))")
                            exit(1)
                        }
                        try? png.write(to: output)
                        print("wrote \(output.path) \(Int(width))x\(Int(height))")
                        exit(0)
                    }
                }
            }
        }
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let web = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: 1200))
web.appearance = NSAppearance(named: dark ? .darkAqua : .aqua)
let loader = Loader()
web.navigationDelegate = loader
let window = NSWindow(contentRect: web.frame, styleMask: [.borderless], backing: .buffered, defer: false)
window.contentView = web
web.loadFileURL(input, allowingReadAccessTo: input.deletingLastPathComponent())
DispatchQueue.main.asyncAfter(deadline: .now() + 45) {
    print("timeout")
    exit(2)
}
app.run()
