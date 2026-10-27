import CoreGraphics
import SnaplineCore
import Vision

/// On-device scan for sensitive text and faces. Nothing leaves the Mac.
enum Redaction {
    /// Returns boxes in view points (top-left origin) for a screen of `viewSize` points.
    static func scan(_ image: CGImage, viewSize: CGSize, categories: Set<SensitiveCategory>) async -> [HideBox] {
        guard !categories.isEmpty else { return [] }
        return await Task.detached(priority: .userInitiated) {
            var boxes: [HideBox] = []
            let handler = VNImageRequestHandler(cgImage: image)

            let text = VNRecognizeTextRequest()
            text.recognitionLevel = .accurate
            text.usesLanguageCorrection = false  // keep tokens and codes as they are
            let faces = VNDetectFaceRectanglesRequest()

            var requests: [VNRequest] = []
            if !categories.subtracting([.face]).isEmpty { requests.append(text) }
            if categories.contains(.face) { requests.append(faces) }
            try? handler.perform(requests)

            for observation in text.results ?? [] {
                guard let candidate = observation.topCandidates(1).first else { continue }
                for match in SensitiveText.matches(in: candidate.string, categories: categories) {
                    guard let box = matchBox(match.range, in: candidate) else { continue }
                    let r = SelectionGeometry.viewRect(fromNormalized: box, viewSize: viewSize).insetBy(dx: -3, dy: -2)
                    boxes.append(HideBox(rect: r, category: match.category))
                }
            }
            for face in faces.results ?? [] {
                // Vision's face box is tight around the features; grow it to cover hair and chin.
                let r = SelectionGeometry.viewRect(fromNormalized: face.boundingBox, viewSize: viewSize)
                boxes.append(HideBox(rect: r.insetBy(dx: -r.width * 0.2, dy: -r.height * 0.3), category: .face))
            }
            return boxes
        }.value
    }

    /// Vision returns the box of the whole word for part of a word (for example the value in
    /// `KEY=value`). In that case cut the word box down to the matched characters.
    private static func matchBox(_ range: Range<String.Index>, in candidate: VNRecognizedText) -> CGRect? {
        guard let box = try? candidate.boundingBox(for: range)?.boundingBox else { return nil }
        let s = candidate.string
        let wordStart = s[..<range.lowerBound].lastIndex(where: \.isWhitespace).map { s.index(after: $0) } ?? s.startIndex
        let wordEnd = s[range.upperBound...].firstIndex(where: \.isWhitespace) ?? s.endIndex
        guard wordStart < range.lowerBound || range.upperBound < wordEnd,
              let wordBox = try? candidate.boundingBox(for: wordStart..<wordEnd)?.boundingBox,
              abs(wordBox.minX - box.minX) < 0.002, abs(wordBox.width - box.width) < 0.002 else { return box }
        let chars = s.distance(from: wordStart, to: range.lowerBound)..<s.distance(from: wordStart, to: range.upperBound)
        return SelectionGeometry.slice(wordBox, characters: chars, of: s.distance(from: wordStart, to: wordEnd))
    }
}
