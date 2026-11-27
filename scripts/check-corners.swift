// Native corner check (docs/DESIGN.md, "Windows"): captures every on-screen window of a running
// HyperMe with its shadow and checks the pixels just inside each corner of the window's frame.
// Runs on the Mac, not in CI (it needs a screen and the Screen Recording permission for the
// terminal).
//
//   swift scripts/check-corners.swift [--out DIR]
//
// A window macOS draws (native chrome) has rounded corners, so the pixel one point in from each
// corner of its frame is shadow or desktop, never the window's opaque background. The pill is a
// transparent panel: around its capsule there must be nothing, so the same pixels are (nearly)
// transparent. A square backdrop or a square shadow shows as an opaque or dark corner and fails.
// The captures are written to DIR (default: a temporary folder) to look at by eye.

import AppKit
import CoreGraphics
import Foundation

struct Window {
    let id: Int
    let name: String
    let bounds: CGRect
}

func hypermeWindows() -> [Window] {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    return list.compactMap { info in
        guard let owner = info[kCGWindowOwnerName as String] as? String, owner.lowercased() == "hyperme",
              let id = info[kCGWindowNumber as String] as? Int,
              let layer = info[kCGWindowLayer as String] as? Int,
              let b = info[kCGWindowBounds as String] as? [String: CGFloat] else { return nil }
        let bounds = CGRect(x: b["X"] ?? 0, y: b["Y"] ?? 0, width: b["Width"] ?? 0, height: b["Height"] ?? 0)
        // The menu-bar item is a window too; skip it and anything tiny.
        if bounds.width < 10 || bounds.height < 10 || (layer > 0 && bounds.height <= 24 && bounds.minY < 30) { return nil }
        let name = (info[kCGWindowName as String] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "layer \(layer)"
        return Window(id: id, name: name, bounds: bounds)
    }
}

func capture(_ window: Window, to path: String) -> CGImage? {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    // -l: one window, with its shadow; -x: no sound.
    task.arguments = ["-x", "-l", String(window.id), path]
    try? task.run()
    task.waitUntilExit()
    guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil) else { return nil }
    return CGImageSourceCreateImageAtIndex(source, 0, nil)
}

/// Alpha and luminance (0...1) of the pixel at (x, y), origin top-left.
func pixel(_ image: CGImage, _ x: Int, _ y: Int) -> (alpha: Double, luma: Double) {
    var data = [UInt8](repeating: 0, count: 4)
    let space = CGColorSpaceCreateDeviceRGB()
    let ctx = CGContext(data: &data, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4, space: space,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.draw(image, in: CGRect(x: -x, y: y - image.height + 1, width: image.width, height: image.height))
    let a = Double(data[3]) / 255
    let luma = a == 0 ? 0 : (0.299 * Double(data[0]) + 0.587 * Double(data[1]) + 0.114 * Double(data[2])) / 255 / a
    return (a, luma)
}

var out = NSTemporaryDirectory() + "hyperme-corners"
if let i = CommandLine.arguments.firstIndex(of: "--out"), i + 1 < CommandLine.arguments.count {
    out = CommandLine.arguments[i + 1]
}
try? FileManager.default.createDirectory(atPath: out, withIntermediateDirectories: true)

let windows = hypermeWindows()
if windows.isEmpty {
    print("No HyperMe window on screen. Start the app (HYPERME_SHOW_PILL=1 HYPERME_OPEN_HOME=home) first.")
    exit(2)
}
var failed = false
for (n, window) in windows.enumerated() {
    let path = "\(out)/window-\(n)-\(window.id).png"
    guard let image = capture(window, to: path) else {
        print("FAIL \(window.name): no capture (Screen Recording permission?)")
        failed = true
        continue
    }
    // The capture includes the shadow around the frame; find the frame inside it.
    let scale = Double(image.width) / Double(window.bounds.width)
    let shadowX = (Double(image.width) - window.bounds.width * scale) / 2
    let shadowTop = (Double(image.height) - window.bounds.height * scale) / 2
    let isPill = window.bounds.height <= 64 || window.name.lowercased().contains("pill")
    // One point in from each corner of the frame.
    let inset = 1.0 * scale
    let left = Int(shadowX + inset), right = Int(shadowX + window.bounds.width * scale - inset - 1)
    let top = Int(shadowTop + inset), bottom = Int(shadowTop + window.bounds.height * scale - inset - 1)
    let centre = pixel(image, (left + right) / 2, (top + bottom) / 2)
    var corners: [String] = []
    var bad = false
    for (label, x, y) in [("top-left", left, top), ("top-right", right, top), ("bottom-left", left, bottom), ("bottom-right", right, bottom)] {
        let p = pixel(image, x, y)
        corners.append(String(format: "%@ a=%.2f", label, p.alpha))
        // The pill: nothing around the capsule. A native window: the corner is not its own
        // opaque background (it is rounded away, leaving shadow or nothing).
        if isPill ? p.alpha > 0.25 : (p.alpha > 0.95 && abs(p.luma - centre.luma) < 0.08) { bad = true }
    }
    print("\(bad ? "FAIL" : "ok  ") \(isPill ? "pill" : "window") \"\(window.name)\" \(Int(window.bounds.width))x\(Int(window.bounds.height)) · \(corners.joined(separator: ", ")) · \(path)")
    failed = failed || bad
}
exit(failed ? 1 : 0)
