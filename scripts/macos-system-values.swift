// Prints the system colours and control heights AppKit reports on this Mac, light and dark, so
// app/ui/tokens.css can be checked against a new macOS (docs/DESIGN.md, "Sources").
//
//   swift scripts/macos-system-values.swift

import AppKit
let names: [(String, NSColor)] = [("labelColor",.labelColor),("secondaryLabelColor",.secondaryLabelColor),("tertiaryLabelColor",.tertiaryLabelColor),("quaternaryLabelColor",.quaternaryLabelColor),("separatorColor",.separatorColor),("windowBackgroundColor",.windowBackgroundColor),("controlBackgroundColor",.controlBackgroundColor),("underPageBackgroundColor",.underPageBackgroundColor),("textBackgroundColor",.textBackgroundColor),("controlColor",.controlColor),("gridColor",.gridColor),("systemRed",.systemRed),("systemOrange",.systemOrange),("systemYellow",.systemYellow),("systemGreen",.systemGreen),("systemBlue",.systemBlue),("controlAccentColor",.controlAccentColor)]
for app in [NSAppearance.Name.aqua, .darkAqua] {
  print("==", app.rawValue)
  NSAppearance(named: app)!.performAsCurrentDrawingAppearance {
    for (n,c) in names {
      let s = c.usingColorSpace(.sRGB)!
      print(String(format:"%@ #%02X%02X%02X a=%.3f", n, Int(round(s.redComponent*255)), Int(round(s.greenComponent*255)), Int(round(s.blueComponent*255)), s.alphaComponent))
    }
  }
}
print(ProcessInfo.processInfo.operatingSystemVersionString)
for sz in [NSControl.ControlSize.mini,.small,.regular,.large] {
  let b = NSButton(title:"OK", target:nil, action:nil); b.bezelStyle = .push; b.controlSize = sz
  let t = NSTextField(string:"x"); t.controlSize = sz; t.font = .systemFont(ofSize: NSFont.systemFontSize(for: sz))
  let p = NSPopUpButton(frame:.zero, pullsDown:false); p.controlSize = sz; p.addItem(withTitle:"Item")
  print("size \(sz.rawValue): button \(b.fittingSize) textfield \(t.fittingSize) popup \(p.fittingSize) font \(NSFont.systemFontSize(for: sz))")
}
