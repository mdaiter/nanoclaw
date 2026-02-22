import Foundation
import ApplicationServices
import AppKit
import AppAutomationCore

public final class AXDriver {
    public let snapshotBuilder = AXSnapshotBuilder()
    public let resolver = AXElementResolver()
    public let performer = AXActionPerformer()

    public init() {}

    public func connect(appName: String) -> (AXUIElement, pid_t)? {
        guard let runningApp = NSWorkspace.shared.runningApplications.first(where: {
            $0.localizedName == appName || $0.bundleIdentifier?.localizedCaseInsensitiveContains(appName) == true
        }) else {
            return nil
        }
        return (AXUIElementCreateApplication(runningApp.processIdentifier), runningApp.processIdentifier)
    }

    public func observe(appName: String, appElement: AXUIElement, pid: pid_t) -> AutomationSnapshot {
        snapshotBuilder.snapshot(appName: appName, appElement: appElement, pid: pid)
    }

    public func find(appName: String, appElement: AXUIElement, pid: pid_t, selector: AutomationSelector) -> [AutomationElement] {
        let snapshot = observe(appName: appName, appElement: appElement, pid: pid)
        let matches = snapshot.elements.filter { resolver.matches($0, selector: selector) }
        if let limit = selector.limit {
            return Array(matches.prefix(limit))
        }
        return matches
    }

    public func perform(appName: String, appElement: AXUIElement, pid: pid_t, action: AutomationAction) -> AutomationResponse<AnyCodableValue> {
        switch action.action {
        case .click:
            return performClick(appName: appName, appElement: appElement, pid: pid, action: action)
        case .setValue:
            return performSetValue(appName: appName, appElement: appElement, pid: pid, action: action)
        case .pressKey:
            return performPressKey(action: action)
        case .scroll:
            return performScroll(appName: appName, appElement: appElement, pid: pid, action: action)
        case .invoke, .menu, .openUrl, .shortcut:
            return AutomationResponse(success: false, errorCode: .unsupportedAction, message: "Action requires app-specific handler")
        }
    }
    
    private func performPressKey(action: AutomationAction) -> AutomationResponse<AnyCodableValue> {
        guard let keyCode = action.params?["keyCode"]?.value as? Int else {
            return AutomationResponse(success: false, errorCode: .invalidRequest, message: "Missing keyCode parameter")
        }
        var modifiers: CGEventFlags = []
        if let mods = action.params?["modifiers"]?.value as? [String] {
            for mod in mods {
                switch mod.lowercased() {
                case "command", "cmd":
                    modifiers.insert(.maskCommand)
                case "shift":
                    modifiers.insert(.maskShift)
                case "option", "alt":
                    modifiers.insert(.maskAlternate)
                case "control", "ctrl":
                    modifiers.insert(.maskControl)
                default:
                    break
                }
            }
        }
        return performer.pressKey(keyCode: CGKeyCode(keyCode), modifiers: modifiers)
    }
    
    private func performScroll(appName: String, appElement: AXUIElement, pid: pid_t, action: AutomationAction) -> AutomationResponse<AnyCodableValue> {
        let deltaX = action.params?["deltaX"]?.value as? Int ?? 0
        let deltaY = action.params?["deltaY"]?.value as? Int ?? 0
        
        if let elementId = action.elementId, let element = snapshotBuilder.element(for: elementId) {
            return performer.scroll(element: element, deltaX: Int32(deltaX), deltaY: Int32(deltaY))
        }
        if let selector = action.selector {
            let matches = find(appName: appName, appElement: appElement, pid: pid, selector: selector)
            guard let first = matches.first, let element = snapshotBuilder.element(for: first.id) else {
                return AutomationResponse(success: false, errorCode: .elementNotFound, message: "No matching element")
            }
            return performer.scroll(element: element, deltaX: Int32(deltaX), deltaY: Int32(deltaY))
        }
        // Scroll without element context
        return performer.scroll(element: appElement, deltaX: Int32(deltaX), deltaY: Int32(deltaY))
    }

    private func performClick(appName: String, appElement: AXUIElement, pid: pid_t, action: AutomationAction) -> AutomationResponse<AnyCodableValue> {
        if let elementId = action.elementId, let element = snapshotBuilder.element(for: elementId) {
            return performer.click(element: element)
        }
        if let selector = action.selector {
            let matches = find(appName: appName, appElement: appElement, pid: pid, selector: selector)
            guard let first = matches.first, let element = snapshotBuilder.element(for: first.id) else {
                return AutomationResponse(success: false, errorCode: .elementNotFound, message: "No matching element")
            }
            return performer.click(element: element)
        }
        return AutomationResponse(success: false, errorCode: .invalidSelector, message: "Missing selector or elementId")
    }

    private func performSetValue(appName: String, appElement: AXUIElement, pid: pid_t, action: AutomationAction) -> AutomationResponse<AnyCodableValue> {
        guard let text = action.params?["text"]?.value as? String else {
            return AutomationResponse(success: false, errorCode: .invalidRequest, message: "Missing text parameter")
        }
        if let elementId = action.elementId, let element = snapshotBuilder.element(for: elementId) {
            return performer.setValue(element: element, value: text)
        }
        if let selector = action.selector {
            let matches = find(appName: appName, appElement: appElement, pid: pid, selector: selector)
            guard let first = matches.first, let element = snapshotBuilder.element(for: first.id) else {
                return AutomationResponse(success: false, errorCode: .elementNotFound, message: "No matching element")
            }
            return performer.setValue(element: element, value: text)
        }
        return AutomationResponse(success: false, errorCode: .invalidSelector, message: "Missing selector or elementId")
    }
}
