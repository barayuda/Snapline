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
                o.topCandidates(1).first.map { ($0.string, o.boundingBox) }
            }
            return ReadingOrder.text(from: pieces)
        }.value
    }
}
