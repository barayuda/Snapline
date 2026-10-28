import CoreGraphics

/// Turns recognised text pieces into plain text in reading order.
public enum ReadingOrder {
    /// `pieces` use Vision boxes: normalized, origin bottom-left. Rows go top to bottom; pieces whose
    /// vertical centres are within half a line height share a row and are joined left to right.
    public static func text(from pieces: [(String, CGRect)]) -> String {
        var rows: [[(String, CGRect)]] = []
        for piece in pieces.sorted(by: { $0.1.midY > $1.1.midY }) {
            if let first = rows.last?.first,
               abs(first.1.midY - piece.1.midY) < min(first.1.height, piece.1.height) / 2 {
                rows[rows.count - 1].append(piece)
            } else {
                rows.append([piece])
            }
        }
        return rows
            .map { $0.sorted { $0.1.minX < $1.1.minX }.map(\.0).joined(separator: " ") }
            .joined(separator: "\n")
    }
}
