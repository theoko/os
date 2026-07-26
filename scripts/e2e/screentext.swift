// Read the words off a screendump, with a pixel box per line.
//
// invariants.py needs to know what the screen SAYS, not only what colour it
// is. Every prose bug this project has shipped is invisible to a pixel
// comparison and obvious to a reader: "5 matches for nvda." above three cards,
// "the bridge is offline" above rows that came from the bridge, the same
// document listed once as "Doc:" and once as "Hit:".
//
// macOS ships the recognizer, so this stays a small tool with nothing to
// install. Output is JSON on stdout:
//
//   {"w":1280,"h":800,"lines":[{"text":"Continue","conf":1,"x":…,"y":…,"w":…,"h":…}]}
//
// Coordinates are top-left-origin pixels of the image, because every other
// coordinate in the suite is, and a harness that mixes origins clicks in the
// wrong place and reports it as a dead control.
//
// Named `screentext` rather than `ocr` on purpose: another session is writing
// its own `ocr.swift` in this directory and neither file should silently
// become the other one.

import AppKit
import Foundation
import Vision

let args = CommandLine.arguments
guard args.count > 1 else {
    FileHandle.standardError.write("usage: screentext <image>\n".data(using: .utf8)!)
    exit(2)
}
guard let img = NSImage(contentsOfFile: args[1]),
    let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    FileHandle.standardError.write("screentext: cannot read \(args[1])\n".data(using: .utf8)!)
    exit(2)
}

let req = VNRecognizeTextRequest()
req.recognitionLevel = .accurate
// UI copy is labels, not prose. Language correction rewrote "os" to "is" and
// "caps" to "cases", which turns a reading of the screen into a guess at it —
// and a guess is exactly what this suite exists to remove.
req.usesLanguageCorrection = false
req.minimumTextHeight = 0.008

do {
    try VNImageRequestHandler(cgImage: cg, options: [:]).perform([req])
} catch {
    FileHandle.standardError.write("screentext: \(error)\n".data(using: .utf8)!)
    exit(3)
}

let width = Double(cg.width), height = Double(cg.height)
var lines: [[String: Any]] = []
for obs in req.results ?? [] {
    guard let best = obs.topCandidates(1).first else { continue }
    let b = obs.boundingBox  // normalised, bottom-left origin
    lines.append([
        "text": best.string,
        "conf": best.confidence,
        "x": b.minX * width,
        "y": (1.0 - b.maxY) * height,
        "w": b.width * width,
        "h": b.height * height,
    ])
}
lines.sort {
    let ay = $0["y"] as! Double, by = $1["y"] as! Double
    if abs(ay - by) > 4 { return ay < by }
    return ($0["x"] as! Double) < ($1["x"] as! Double)
}

let payload: [String: Any] = ["w": cg.width, "h": cg.height, "lines": lines]
FileHandle.standardOutput.write(try! JSONSerialization.data(withJSONObject: payload))
