import Foundation
import ApplicationServices
import AppKit
import AppAutomationCore

public final class AXSnapshotBuilder {
    private var elementCache: [String: AXUIElement] = [:]
    private var idCounter: Int = 0

    public init() {}

    public func snapshot(appName: String, appElement: AXUIElement, pid: pid_t) -> AutomationSnapshot {
        elementCache.removeAll()
        idCounter = 0
        let root = inspect(element: appElement, path: "")
        let elements = flatten(root)
        let focused = elements.first(where: { $0.focused })?.id
        let hash = elements.map { "\($0.id):\($0.title ?? ""): \($0.value ?? "")" }.joined().hashValue
        let timestamp = ISO8601DateFormatter().string(from: Date())
        return AutomationSnapshot(
            timestamp: timestamp,
            appName: appName,
            pid: pid,
            focusedElement: focused,
            elements: elements,
            hash: String(hash)
        )
    }

    public func element(for id: String) -> AXUIElement? {
        elementCache[id]
    }

    private struct AXNode {
        let element: AutomationElement
        let children: [AXNode]
    }

    private func inspect(element: AXUIElement, path: String) -> AXNode {
        let id = "e\(idCounter)"
        idCounter += 1
        elementCache[id] = element

        let role = getStringAttr(element, kAXRoleAttribute) ?? "Unknown"
        let title = getStringAttr(element, kAXTitleAttribute)
        var value = getStringAttr(element, kAXValueAttribute)
        if let v = value, v.count > 200 {
            value = String(v.prefix(200)) + "..."
        }
        let enabled = getBoolAttr(element, kAXEnabledAttribute) ?? true
        let focused = getBoolAttr(element, kAXFocusedAttribute) ?? false
        let actions = getActions(element)
        let bounds = getBounds(element)

        let currentPath = path.isEmpty ? role : "\(path) > \(role)"
        var children: [AXNode] = []
        if let childElements = getChildren(element) {
            children = childElements.prefix(100).map { inspect(element: $0, path: currentPath) }
        }

        let node = AutomationElement(
            id: id,
            role: role,
            title: title,
            value: value,
            enabled: enabled,
            focused: focused,
            path: currentPath,
            bounds: bounds,
            actions: actions
        )

        return AXNode(element: node, children: children)
    }

    private func flatten(_ node: AXNode) -> [AutomationElement] {
        var result = [node.element]
        for child in node.children {
            result.append(contentsOf: flatten(child))
        }
        return result
    }

    private func getChildren(_ element: AXUIElement) -> [AXUIElement]? {
        var childrenRef: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &childrenRef) == .success,
              let children = childrenRef as? [AXUIElement] else {
            return nil
        }
        return children
    }

    private func getActions(_ element: AXUIElement) -> [String] {
        var names: CFArray?
        guard AXUIElementCopyActionNames(element, &names) == .success else { return [] }
        return (names as? [String]) ?? []
    }

    private func getStringAttr(_ element: AXUIElement, _ attr: String) -> String? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, attr as CFString, &value) == .success else { return nil }
        return value as? String
    }

    private func getBoolAttr(_ element: AXUIElement, _ attr: String) -> Bool? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, attr as CFString, &value) == .success else { return nil }
        return value as? Bool
    }

    private func getBounds(_ element: AXUIElement) -> AutomationBounds? {
        var value: CFTypeRef?
        let frameAttr = "AXFrame"
        guard AXUIElementCopyAttributeValue(element, frameAttr as CFString, &value) == .success else {
            return nil
        }
        let frameValue = unsafeBitCast(value, to: AXValue.self)
        var rect = CGRect.zero
        if AXValueGetType(frameValue) == .cgRect, AXValueGetValue(frameValue, .cgRect, &rect) {
            return AutomationBounds(minX: rect.minX, minY: rect.minY, maxX: rect.maxX, maxY: rect.maxY)
        }
        return nil
    }
}
