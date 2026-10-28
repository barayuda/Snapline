import CoreGraphics
import SnaplineCore
import Vision

/// On-device text recognition for "Copy Text". Nothing leaves the Mac.
enum TextExtraction {
    /// Plain text in reading order, or "" when the image has no readable text.
    static func text(in image: CGImage) async -> String {
        await Task.detached(priority: .userInitiated) {
            let request = VNRecognizeTextRequest()
            request.recognitionLevel = .accurate
            request.usesLanguageCorrection = false  // keeps codes, IDs and numbers exact (correction turned "7" into "Z")
            request.automaticallyDetectsLanguage = true
            try? VNImageRequestHandler(cgImage: image).perform([request])
            let pieces = (request.results ?? []).compactMap { o in
                o.topCandidates(1).first.map { (repairSymbols($0, in: image), o.boundingBox) }
            }
            return ReadingOrder.text(from: pieces)
        }.value
    }

    /// Vision cannot read ⌘ ⇧ ⌥ ⌃ and similar symbols; check each word that might contain one.
    private static func repairSymbols(_ candidate: VNRecognizedText, in image: CGImage) -> String {
        var text = candidate.string
        let words = text.split(separator: " ").filter(SymbolRepair.hasSuspects)
        // Replace from the end so earlier ranges stay valid.
        for word in words.reversed() {
            let range = word.startIndex..<word.endIndex
            guard let box = try? candidate.boundingBox(for: range)?.boundingBox,
                  let crop = image.cropping(to: pixelRect(box, image).insetBy(dx: -2, dy: -2)) else { continue }
            text.replaceSubrange(range, with: SymbolRepair.repair(String(word), in: crop))
        }
        return text
    }

    /// Vision's normalized box (origin bottom-left) → pixel rect in the image (origin top-left).
    private static func pixelRect(_ box: CGRect, _ image: CGImage) -> CGRect {
        let w = CGFloat(image.width), h = CGFloat(image.height)
        return CGRect(x: box.minX * w, y: (1 - box.maxY) * h, width: box.width * w, height: box.height * h).integral
    }
}
