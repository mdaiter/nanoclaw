import Foundation

public enum AutomationDiffBuilder {
    public static func build(previous: AutomationSnapshot, current: AutomationSnapshot) -> AutomationDiff {
        let prevSet = Set(previous.elements)
        let currSet = Set(current.elements)

        let added = currSet.subtracting(prevSet)
        let removed = prevSet.subtracting(currSet)

        var modified: [AutomationDiff.ElementChange] = []
        let prevById = Dictionary(uniqueKeysWithValues: previous.elements.map { ($0.id, $0) })
        let currById = Dictionary(uniqueKeysWithValues: current.elements.map { ($0.id, $0) })

        for (id, curr) in currById {
            if let prev = prevById[id] {
                if prev.title != curr.title {
                    modified.append(.init(id: id, field: "title", before: prev.title, after: curr.title))
                }
                if prev.value != curr.value {
                    modified.append(.init(id: id, field: "value", before: prev.value, after: curr.value))
                }
                if prev.enabled != curr.enabled {
                    modified.append(.init(id: id, field: "enabled", before: String(prev.enabled), after: String(curr.enabled)))
                }
                if prev.focused != curr.focused {
                    modified.append(.init(id: id, field: "focused", before: String(prev.focused), after: String(curr.focused)))
                }
            }
        }

        let changed = !added.isEmpty || !removed.isEmpty || !modified.isEmpty
        var summaryParts: [String] = []
        if !added.isEmpty { summaryParts.append("\(added.count) added") }
        if !removed.isEmpty { summaryParts.append("\(removed.count) removed") }
        if !modified.isEmpty { summaryParts.append("\(modified.count) modified") }

        let summary = changed ? "UI changed: \(summaryParts.joined(separator: ", "))." : "No changes detected."

        return AutomationDiff(
            changed: changed,
            added: Array(added),
            removed: Array(removed),
            modified: modified,
            signals: [],
            summary: summary
        )
    }
}
