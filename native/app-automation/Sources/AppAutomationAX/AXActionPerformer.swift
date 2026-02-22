import Foundation
import ApplicationServices
import AppKit
import AppAutomationCore
import Carbon.HIToolbox

public final class AXActionPerformer {
    public init() {}

    public func click(element: AXUIElement) -> AutomationResponse<AnyCodableValue> {
        let result = AXUIElementPerformAction(element, kAXPressAction as CFString)
        if result == .success {
            return AutomationResponse(success: true, message: "Clicked element")
        }
        return AutomationResponse(success: false, errorCode: .actionFailed, message: "Failed to click: AXError \(result.rawValue)")
    }

    public func setValue(element: AXUIElement, value: String) -> AutomationResponse<AnyCodableValue> {
        // Try direct AXValue first
        let result = AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, value as CFTypeRef)
        if result == .success {
            return AutomationResponse(success: true, message: "Set value")
        }
        
        // Fallback: focus + keyboard events
        let focusResult = focus(element: element)
        guard focusResult.success else {
            return AutomationResponse(success: false, errorCode: .actionFailed, message: "Failed to focus for typing")
        }
        
        // Small delay for focus to take effect
        Thread.sleep(forTimeInterval: 0.1)
        
        return typeText(value)
    }

    public func focus(element: AXUIElement) -> AutomationResponse<AnyCodableValue> {
        let result = AXUIElementSetAttributeValue(element, kAXFocusedAttribute as CFString, true as CFTypeRef)
        if result == .success {
            return AutomationResponse(success: true, message: "Focused element")
        }
        return AutomationResponse(success: false, errorCode: .actionFailed, message: "Failed to focus: AXError \(result.rawValue)")
    }
    
    public func typeText(_ text: String) -> AutomationResponse<AnyCodableValue> {
        let source = CGEventSource(stateID: .hidSystemState)
        
        for char in text {
            guard let keyEvent = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true) else {
                continue
            }
            
            var chars = [UniChar](String(char).utf16)
            keyEvent.keyboardSetUnicodeString(stringLength: chars.count, unicodeString: &chars)
            keyEvent.post(tap: .cghidEventTap)
            
            let keyUpEvent = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false)
            keyUpEvent?.post(tap: .cghidEventTap)
            
            // Small delay between keystrokes
            Thread.sleep(forTimeInterval: 0.01)
        }
        
        return AutomationResponse(success: true, message: "Typed text via keyboard events")
    }
    
    public func pressKey(keyCode: CGKeyCode, modifiers: CGEventFlags = []) -> AutomationResponse<AnyCodableValue> {
        let source = CGEventSource(stateID: .hidSystemState)
        
        guard let keyDown = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: true),
              let keyUp = CGEvent(keyboardEventSource: source, virtualKey: keyCode, keyDown: false) else {
            return AutomationResponse(success: false, errorCode: .actionFailed, message: "Failed to create key event")
        }
        
        keyDown.flags = modifiers
        keyUp.flags = modifiers
        
        keyDown.post(tap: .cghidEventTap)
        keyUp.post(tap: .cghidEventTap)
        
        return AutomationResponse(success: true, message: "Key pressed")
    }
    
    public func scroll(element: AXUIElement, deltaX: Int32, deltaY: Int32) -> AutomationResponse<AnyCodableValue> {
        guard let scrollEvent = CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 2, wheel1: deltaY, wheel2: deltaX, wheel3: 0) else {
            return AutomationResponse(success: false, errorCode: .actionFailed, message: "Failed to create scroll event")
        }
        scrollEvent.post(tap: .cghidEventTap)
        return AutomationResponse(success: true, message: "Scrolled")
    }
}
