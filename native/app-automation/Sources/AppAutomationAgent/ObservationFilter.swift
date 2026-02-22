import Foundation
import AppAutomationCore

/// Filters UI snapshots to reduce context size for LLM consumption
struct ObservationFilter {
    /// Maximum characters for element values
    private let maxValueLength = 100
    
    /// Interactive roles we care about
    private let interactiveRoles: Set<String> = [
        "AXButton",
        "AXTextField",
        "AXTextArea",
        "AXCheckBox",
        "AXRadioButton",
        "AXPopUpButton",
        "AXMenuItem",
        "AXMenuButton",
        "AXMenu",
        "AXMenuBar",
        "AXMenuBarItem",
        "AXLink",
        "AXComboBox",
        "AXSlider",
        "AXTabGroup",
        "AXTab",
        "AXToolbar",
        "AXList",
        "AXTable",
        "AXOutline",
        "AXRow",
        "AXCell",
        "AXScrollArea",
        "AXSplitGroup",
        "AXImage",  // Sometimes clickable
        "AXStaticText",  // For context
        "AXWindow",
        "AXSheet",
        "AXDialog",
        "AXGroup",  // Often contains interactive elements
    ]
    
    /// Roles to always include for context
    private let contextRoles: Set<String> = [
        "AXWindow",
        "AXSheet",
        "AXDialog",
        "AXToolbar",
        "AXTabGroup",
    ]
    
    func filter(snapshot: AutomationSnapshot) -> FilteredSnapshot {
        var filtered: [FilteredElement] = []
        var focusedElement: FilteredElement?
        
        for element in snapshot.elements {
            // Always include focused element
            if element.focused {
                let filteredElement = filterElement(element)
                focusedElement = filteredElement
                filtered.append(filteredElement)
                continue
            }
            
            // Include interactive elements
            if interactiveRoles.contains(element.role) {
                // Skip empty groups and containers
                if element.role == "AXGroup" && element.title == nil && element.value == nil {
                    continue
                }
                
                // Skip static text that's too long (probably body text)
                if element.role == "AXStaticText" {
                    if let value = element.value, value.count > maxValueLength {
                        continue
                    }
                }
                
                filtered.append(filterElement(element))
            }
        }
        
        // Sort by path depth (shallower first) then by ID
        filtered.sort { a, b in
            let depthA = a.path.components(separatedBy: " > ").count
            let depthB = b.path.components(separatedBy: " > ").count
            if depthA != depthB { return depthA < depthB }
            return a.id < b.id
        }
        
        return FilteredSnapshot(
            appName: snapshot.appName,
            timestamp: snapshot.timestamp,
            elementCount: filtered.count,
            totalElements: snapshot.elements.count,
            focusedElement: focusedElement,
            elements: filtered
        )
    }
    
    private func filterElement(_ element: AutomationElement) -> FilteredElement {
        var truncatedValue = element.value
        if let value = truncatedValue, value.count > maxValueLength {
            truncatedValue = String(value.prefix(maxValueLength)) + "..."
        }
        
        return FilteredElement(
            id: element.id,
            role: simplifyRole(element.role),
            title: element.title,
            value: truncatedValue,
            enabled: element.enabled,
            focused: element.focused,
            path: simplifyPath(element.path),
            actions: element.actions.isEmpty ? nil : element.actions
        )
    }
    
    /// Simplify AX role names for readability
    private func simplifyRole(_ role: String) -> String {
        if role.hasPrefix("AX") {
            return String(role.dropFirst(2))
        }
        return role
    }
    
    /// Simplify path for readability
    private func simplifyPath(_ path: String) -> String {
        path.replacingOccurrences(of: "AX", with: "")
    }
}

// MARK: - Filtered Snapshot Types

struct FilteredSnapshot: Codable {
    let appName: String
    let timestamp: String
    let elementCount: Int
    let totalElements: Int
    let focusedElement: FilteredElement?
    let elements: [FilteredElement]
    
    func asText() -> String {
        var lines: [String] = []
        lines.append("App: \(appName)")
        lines.append("Elements: \(elementCount) shown (of \(totalElements) total)")
        
        if let focused = focusedElement {
            lines.append("Focused: \(focused.summary)")
        }
        
        lines.append("")
        lines.append("UI Elements:")
        for element in elements {
            lines.append("  [\(element.id)] \(element.summary)")
        }
        
        return lines.joined(separator: "\n")
    }
}

struct FilteredElement: Codable {
    let id: String
    let role: String
    let title: String?
    let value: String?
    let enabled: Bool
    let focused: Bool
    let path: String
    let actions: [String]?
    
    var summary: String {
        var parts: [String] = [role]
        
        if let title = title, !title.isEmpty {
            parts.append("'\(title)'")
        }
        
        if let value = value, !value.isEmpty {
            parts.append("value='\(value)'")
        }
        
        if focused {
            parts.append("[FOCUSED]")
        }
        
        if !enabled {
            parts.append("[disabled]")
        }
        
        return parts.joined(separator: " ")
    }
}
