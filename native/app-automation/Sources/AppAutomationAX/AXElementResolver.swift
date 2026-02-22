import Foundation
import ApplicationServices
import AppAutomationCore

public struct AXElementResolver {
    public init() {}

    public func matches(_ element: AutomationElement, selector: AutomationSelector) -> Bool {
        if let role = selector.role, !matchesField(role, element.role) {
            return false
        }
        if let title = selector.title {
            let value = element.title ?? ""
            if !matchesField(title, value) {
                return false
            }
        }
        if let value = selector.value {
            let current = element.value ?? ""
            if !matchesField(value, current) {
                return false
            }
        }
        if let path = selector.path, !matchesField(path, element.path) {
            return false
        }
        if let enabled = selector.enabled, enabled != element.enabled {
            return false
        }
        if let focused = selector.focused, focused != element.focused {
            return false
        }
        if let bounds = selector.bounds, let elementBounds = element.bounds {
            if let minX = bounds.minX, elementBounds.minX < minX { return false }
            if let minY = bounds.minY, elementBounds.minY < minY { return false }
            if let maxX = bounds.maxX, elementBounds.maxX > maxX { return false }
            if let maxY = bounds.maxY, elementBounds.maxY > maxY { return false }
        }
        return true
    }

    private func matchesField(_ field: SelectorField, _ value: String) -> Bool {
        switch field.match {
        case .exact:
            return value == field.value
        case .contains:
            return value.localizedCaseInsensitiveContains(field.value)
        }
    }
}
