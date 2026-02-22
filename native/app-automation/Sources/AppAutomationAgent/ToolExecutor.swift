import Foundation
import CoreGraphics
import ApplicationServices
import AppKit
import AppAutomationCore
import AppAutomationAX

/// Executes tools called by Claude
actor ToolExecutor {
    private let driver = AXDriver()
    private let filter = ObservationFilter()
    private let appName: String
    private var appElement: AXUIElement?
    private var pid: pid_t = 0
    private var runningApp: NSRunningApplication?
    
    init(appName: String) {
        self.appName = appName
    }
    
    func connect() -> Bool {
        guard let (element, processId) = driver.connect(appName: appName) else {
            return false
        }
        self.appElement = element
        self.pid = processId
        
        // Find and store the running app reference
        self.runningApp = NSWorkspace.shared.runningApplications.first(where: {
            $0.processIdentifier == processId
        })
        
        // Activate the app
        activateApp()
        
        return true
    }
    
    /// Activate the app to bring it to foreground
    private func activateApp() {
        runningApp?.activate(options: [.activateIgnoringOtherApps])
        // Small delay for activation to complete
        Thread.sleep(forTimeInterval: 0.1)
    }
    
    func observe() -> FilteredSnapshot? {
        guard let appElement else { return nil }
        let snapshot = driver.observe(appName: appName, appElement: appElement, pid: pid)
        return filter.filter(snapshot: snapshot)
    }
    
    func execute(toolUse: ClaudeToolUse) async -> ToolResult {
        // Ensure app is active before executing actions (except observe and wait)
        let needsActivation = ["click", "type_text", "press_key", "scroll", "focus", "open_url"].contains(toolUse.name)
        if needsActivation {
            activateApp()
        }
        
        switch toolUse.name {
        case "observe_ui":
            return executeObserve()
        case "click":
            return executeClick(input: toolUse.input)
        case "type_text":
            return executeType(input: toolUse.input)
        case "press_key":
            return executePressKey(input: toolUse.input)
        case "open_url":
            return executeOpenURL(input: toolUse.input)
        case "wait":
            return await executeWait(input: toolUse.input)
        case "scroll":
            return executeScroll(input: toolUse.input)
        case "focus":
            return executeFocus(input: toolUse.input)
        default:
            return ToolResult(success: false, message: "Unknown tool: \(toolUse.name)")
        }
    }
    
    // MARK: - Tool Implementations
    
    private func executeObserve() -> ToolResult {
        guard let snapshot = observe() else {
            return ToolResult(success: false, message: "Failed to observe UI - app may not be running")
        }
        return ToolResult(success: true, message: snapshot.asText())
    }
    
    private func executeClick(input: [String: JSONValue]) -> ToolResult {
        guard let appElement else {
            return ToolResult(success: false, message: "Not connected to app")
        }
        
        // Try by element ID first
        if let elementId = input["element_id"]?.stringValue {
            guard let element = driver.snapshotBuilder.element(for: elementId) else {
                return ToolResult(success: false, message: "Element '\(elementId)' not found")
            }
            let response = driver.performer.click(element: element)
            return ToolResult(success: response.success, message: response.message)
        }
        
        // Try by selector
        let selector = buildSelector(from: input)
        let action = AutomationAction(action: .click, selector: selector)
        let response = driver.perform(appName: appName, appElement: appElement, pid: pid, action: action)
        return ToolResult(success: response.success, message: response.message)
    }
    
    private func executeType(input: [String: JSONValue]) -> ToolResult {
        guard appElement != nil else {
            return ToolResult(success: false, message: "Not connected to app")
        }
        
        guard let text = input["text"]?.stringValue else {
            return ToolResult(success: false, message: "Missing 'text' parameter")
        }
        
        let clearFirst = input["clear_first"]?.stringValue == "true"
        
        // If element_id specified, focus it first
        if let elementId = input["element_id"]?.stringValue {
            guard let element = driver.snapshotBuilder.element(for: elementId) else {
                return ToolResult(success: false, message: "Element '\(elementId)' not found")
            }
            let focusResult = driver.performer.focus(element: element)
            if !focusResult.success {
                return ToolResult(success: false, message: "Failed to focus element: \(focusResult.message)")
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
        
        // Clear field if requested
        if clearFirst {
            // Select all (cmd+a) then delete
            _ = driver.performer.pressKey(keyCode: 0, modifiers: .maskCommand) // cmd+a
            Thread.sleep(forTimeInterval: 0.05)
            _ = driver.performer.pressKey(keyCode: 51, modifiers: []) // delete
            Thread.sleep(forTimeInterval: 0.05)
        }
        
        // Type the text
        let response = driver.performer.typeText(text)
        return ToolResult(success: response.success, message: response.message)
    }
    
    private func executePressKey(input: [String: JSONValue]) -> ToolResult {
        guard let keyName = input["key"]?.stringValue else {
            return ToolResult(success: false, message: "Missing 'key' parameter")
        }
        
        guard let keyCode = keyCodeFor(keyName) else {
            return ToolResult(success: false, message: "Unknown key: '\(keyName)'")
        }
        
        var modifiers: CGEventFlags = []
        if let mods = input["modifiers"]?.stringArrayValue {
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
        
        let response = driver.performer.pressKey(keyCode: keyCode, modifiers: modifiers)
        let keyDesc = modifiers.isEmpty ? keyName : "\(modifierString(modifiers))+\(keyName)"
        return ToolResult(
            success: response.success,
            message: response.success ? "Pressed \(keyDesc)" : response.message
        )
    }
    
    private func executeOpenURL(input: [String: JSONValue]) -> ToolResult {
        guard var urlString = input["url"]?.stringValue else {
            return ToolResult(success: false, message: "Missing 'url' parameter")
        }
        
        // Add https:// if no scheme
        if !urlString.contains("://") {
            urlString = "https://\(urlString)"
        }
        
        // Use AppleScript to open URL in the app
        let script = "tell application \"\(appName)\" to open location \"\(urlString)\""
        let result = runAppleScript(script)
        return result
    }
    
    private func executeWait(input: [String: JSONValue]) async -> ToolResult {
        let ms = input["ms"]?.intValue ?? 1000
        try? await Task.sleep(nanoseconds: UInt64(ms) * 1_000_000)
        return ToolResult(success: true, message: "Waited \(ms)ms")
    }
    
    private func executeScroll(input: [String: JSONValue]) -> ToolResult {
        guard let appElement else {
            return ToolResult(success: false, message: "Not connected to app")
        }
        
        guard let direction = input["direction"]?.stringValue else {
            return ToolResult(success: false, message: "Missing 'direction' parameter")
        }
        
        let amount = Int32(input["amount"]?.intValue ?? 200)
        
        var deltaX: Int32 = 0
        var deltaY: Int32 = 0
        
        switch direction {
        case "up":
            deltaY = amount
        case "down":
            deltaY = -amount
        case "left":
            deltaX = amount
        case "right":
            deltaX = -amount
        default:
            return ToolResult(success: false, message: "Invalid direction: '\(direction)'")
        }
        
        let response = driver.performer.scroll(element: appElement, deltaX: deltaX, deltaY: deltaY)
        return ToolResult(success: response.success, message: response.success ? "Scrolled \(direction)" : response.message)
    }
    
    private func executeFocus(input: [String: JSONValue]) -> ToolResult {
        guard let elementId = input["element_id"]?.stringValue else {
            return ToolResult(success: false, message: "Missing 'element_id' parameter")
        }
        
        guard let element = driver.snapshotBuilder.element(for: elementId) else {
            return ToolResult(success: false, message: "Element '\(elementId)' not found")
        }
        
        let response = driver.performer.focus(element: element)
        return ToolResult(success: response.success, message: response.message)
    }
    
    // MARK: - Helpers
    
    private func buildSelector(from input: [String: JSONValue]) -> AutomationSelector {
        var selector = AutomationSelector()
        
        if let role = input["role"]?.stringValue {
            let normalizedRole = role.hasPrefix("AX") ? role : "AX\(role)"
            selector.role = SelectorField(value: normalizedRole, match: .exact)
        }
        
        if let title = input["title"]?.stringValue {
            selector.title = SelectorField(value: title, match: .exact)
        } else if let titleContains = input["title_contains"]?.stringValue {
            selector.title = SelectorField(value: titleContains, match: .contains)
        }
        
        if let value = input["value"]?.stringValue {
            selector.value = SelectorField(value: value, match: .contains)
        }
        
        return selector
    }
    
    private func keyCodeFor(_ keyName: String) -> CGKeyCode? {
        let name = keyName.lowercased()
        
        // Special keys
        switch name {
        case "return", "enter": return 36
        case "tab": return 48
        case "space": return 49
        case "delete", "backspace": return 51
        case "escape", "esc": return 53
        case "up": return 126
        case "down": return 125
        case "left": return 123
        case "right": return 124
        case "home": return 115
        case "end": return 119
        case "pageup": return 116
        case "pagedown": return 121
        case "[": return 33
        case "]": return 30
        case "f1": return 122
        case "f2": return 120
        case "f3": return 99
        case "f4": return 118
        case "f5": return 96
        case "f6": return 97
        case "f7": return 98
        case "f8": return 100
        case "f9": return 101
        case "f10": return 109
        case "f11": return 103
        case "f12": return 111
        default: break
        }
        
        // Letters
        let letterCodes: [Character: CGKeyCode] = [
            "a": 0, "b": 11, "c": 8, "d": 2, "e": 14, "f": 3, "g": 5, "h": 4,
            "i": 34, "j": 38, "k": 40, "l": 37, "m": 46, "n": 45, "o": 31, "p": 35,
            "q": 12, "r": 15, "s": 1, "t": 17, "u": 32, "v": 9, "w": 13, "x": 7,
            "y": 16, "z": 6
        ]
        
        if let char = name.first, let code = letterCodes[char] {
            return code
        }
        
        // Numbers
        let numberCodes: [Character: CGKeyCode] = [
            "0": 29, "1": 18, "2": 19, "3": 20, "4": 21,
            "5": 23, "6": 22, "7": 26, "8": 28, "9": 25
        ]
        
        if let char = name.first, let code = numberCodes[char] {
            return code
        }
        
        return nil
    }
    
    private func modifierString(_ flags: CGEventFlags) -> String {
        var mods: [String] = []
        if flags.contains(.maskCommand) { mods.append("cmd") }
        if flags.contains(.maskShift) { mods.append("shift") }
        if flags.contains(.maskAlternate) { mods.append("opt") }
        if flags.contains(.maskControl) { mods.append("ctrl") }
        return mods.joined(separator: "+")
    }
    
    private func runAppleScript(_ script: String) -> ToolResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", script]
        let output = Pipe()
        process.standardOutput = output
        process.standardError = output
        
        do {
            try process.run()
        } catch {
            return ToolResult(success: false, message: "Failed to run AppleScript: \(error.localizedDescription)")
        }
        
        process.waitUntilExit()
        let data = output.fileHandleForReading.readDataToEndOfFile()
        let response = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        
        if process.terminationStatus != 0 {
            return ToolResult(success: false, message: response.isEmpty ? "AppleScript failed" : response)
        }
        
        return ToolResult(success: true, message: response.isEmpty ? "OK" : response)
    }
}

struct ToolResult {
    let success: Bool
    let message: String
}

// AXDriver now has public snapshotBuilder and performer properties
