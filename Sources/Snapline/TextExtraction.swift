import CoreGraphics
import Foundation
import SnaplineCore
import Vision

/// On-device text recognition for "Copy Text". Nothing leaves the Mac.
enum TextExtraction {
    /// Text in reading order, or "" when the image has no readable text. Spots the user should check
    /// are wrapped in `UncertainMarks`; strip them (or show the review panel) before copying.
    static func text(in image: CGImage) async -> String {
        await Task.detached(priority: .userInitiated) {
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = false  // keeps codes, IDs and numbers exact (correction turned "7" into "Z")
            // The Mac's own languages; automatic detection read symbol-heavy lines as Chinese.
            request.recognitionLanguages = RecognitionLanguages.pick(
                preferred: Locale.preferredLanguages,
                supported: (try? request.supportedRecognitionLanguages()) ?? ["en-US"])
            try? VNImageRequestHandler(cgImage: image).perform([request])
            let pieces = (request.results ?? []).compactMap { o in
                o.topCandidates(1).first.map { (line($0, box: o.boundingBox, in: image), o.boundingBox) }
            }
            return ReadingOrder.text(from: pieces)
        }.value
    }

    /// One recognised line: repair misread keyboard symbols in each word, and put back glyphs
    /// that text recognition skipped (⌘, ↩… are sometimes dropped entirely).
    private static func line(_ candidate: VNRecognizedText, box: CGRect, in image: CGImage) -> String {
        let s = candidate.string
        let ranges = s.split(separator: " ").map { $0.startIndex..<$0.endIndex }
        let lineRect = pixelRect(box, image).insetBy(dx: -2, dy: -2)
            .intersection(CGRect(x: 0, y: 0, width: image.width, height: image.height))

        var words: [String] = []
        var spans: [Range<Int>] = []
        for range in ranges {
            let word = String(s[range])
            let wordRect = (try? candidate.boundingBox(for: range)?.boundingBox).map { pixelRect($0, image) }
            if let r = wordRect {
                spans.append(max(0, Int(r.minX - lineRect.minX))..<max(0, Int(r.maxX - lineRect.minX)))
            }
            guard SymbolRepair.hasSuspects(s[range]), let r = wordRect,
                  let crop = image.cropping(to: r.insetBy(dx: -2, dy: -2)) else {
                words.append(word)
                continue
            }
            let result = SymbolRepair.check(word, in: crop)
            words.append(result.uncertain ? UncertainMarks.wrap(result.text) : result.text)
        }

        // Only look for skipped glyphs when every word has a box; a missing box would look like a gap.
        if spans.count == ranges.count, let lineCrop = image.cropping(to: lineRect) {
            for dropped in SymbolRepair.droppedSymbols(in: lineCrop, keptWords: spans).reversed() {
                words.insert(dropped.symbol ?? UncertainMarks.wrap("\u{FFFD}"), at: dropped.afterWord + 1)
            }
        }
        return words.joined(separator: " ")
    }

    /// Vision's normalized box (origin bottom-left) → pixel rect in the image (origin top-left).
    private static func pixelRect(_ box: CGRect, _ image: CGImage) -> CGRect {
        let w = CGFloat(image.width), h = CGFloat(image.height)
        return CGRect(x: box.minX * w, y: (1 - box.maxY) * h, width: box.width * w, height: box.height * h).integral
    }
}
