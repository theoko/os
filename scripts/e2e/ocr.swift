// Read the text off a screenshot, with per-line pixel boxes.
//
// The arm64 guest has no QMP and no serial, so a screenshot is the only thing
// that comes back from it. A screenshot alone can prove pixels were drawn; it
// cannot prove the *right* pixels were drawn, which is where every UI bug this
// project has shipped lives ("5 matches" above three cards; the same document
// listed as both "Doc:" and "Hit:"). Reading the text turns the screenshot into
// something assertions can be written against.
//
// Vision ships with macOS, so this needs no third-party OCR. Output is JSON on
// stdout: {"width":W,"height":H,"lines":[{"text","conf","x","y","w","h"},...]}
// sorted top-to-bottom, left-to-right, in pixel coordinates of the image.

import AppKit
import Foundation
import Vision

let args = CommandLine.arguments
guard args.count > 1 else {
    FileHandle.standardError.write("usage: ocrtool <image.png>\n".data(using: .utf8)!)
    exit(2)
}
guard let img = NSImage(contentsOfFile: args[1]),
      let cg = img.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    FileHandle.standardError.write("cannot read image: \(args[1])\n".data(using: .utf8)!)
    exit(2)
}

let W = Double(cg.width)
let H = Double(cg.height)

let req = VNRecognizeTextRequest()
req.recognitionLevel = .accurate
// The UI is full of strings no dictionary knows ("nvda", "tsearch.sync",
// "os://"). Language correction rewrote them into real words, which made the
// assertions read as failures against a screen that was correct.
req.usesLanguageCorrection = false

do {
    try VNImageRequestHandler(cgImage: cg, options: [:]).perform([req])
} catch {
    FileHandle.standardError.write("vision failed: \(error)\n".data(using: .utf8)!)
    exit(3)
}

var lines: [[String: Any]] = []
for obs in (req.results ?? []) {
    guard let top = obs.topCandidates(1).first else { continue }
    // Vision reports a normalised box with the origin at the bottom-left;
    // everything else here counts pixels down from the top-left.
    let b = obs.boundingBox
    lines.append([
        "text": top.string,
        "conf": Double(top.confidence),
        "x": Int(b.minX * W),
        "y": Int((1.0 - b.maxY) * H),
        "w": Int(b.width * W),
        "h": Int(b.height * H),
    ])
}
lines.sort {
    let ay = $0["y"] as! Int, by = $1["y"] as! Int
    return ay == by ? ($0["x"] as! Int) < ($1["x"] as! Int) : ay < by
}

let payload: [String: Any] = ["width": Int(W), "height": Int(H), "lines": lines]
let data = try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
FileHandle.standardOutput.write(data)
