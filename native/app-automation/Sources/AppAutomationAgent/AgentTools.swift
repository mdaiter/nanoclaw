import Foundation

/// Tool definitions for Claude
enum AgentTools {
    static let allTools: [ClaudeTool] = [
        observeUI,
        click,
        typeText,
        pressKey,
        openURL,
        wait,
        scroll,
        focus
    ]
    
    static let observeUI = ClaudeTool(
        name: "observe_ui",
        description: "Get the current UI state of the app. Returns a filtered list of interactive elements with their IDs, roles, titles, and values. Use this to understand what's on screen before taking action.",
        input_schema: .object(properties: [:])
    )
    
    static let click = ClaudeTool(
        name: "click",
        description: "Click on a UI element. You can specify the element by ID (from observe_ui) or by a selector matching role/title/value.",
        input_schema: .object(
            properties: [
                "element_id": .string("The element ID from observe_ui (e.g., 'e42')"),
                "role": .string("Optional: Element role to match (e.g., 'Button', 'TextField')"),
                "title": .string("Optional: Element title to match (exact or partial)"),
                "title_contains": .string("Optional: Partial title match"),
                "value": .string("Optional: Element value to match"),
            ],
            required: []
        )
    )
    
    static let typeText = ClaudeTool(
        name: "type_text",
        description: "Type text into the currently focused element or a specified element. The element should be a text field or text area.",
        input_schema: .object(
            properties: [
                "text": .string("The text to type"),
                "element_id": .string("Optional: Target element ID. If not specified, types into the focused element."),
                "clear_first": .string("Optional: Set to 'true' to clear the field before typing"),
            ],
            required: ["text"]
        )
    )
    
    static let pressKey = ClaudeTool(
        name: "press_key",
        description: "Press a keyboard key with optional modifiers. Use this for keyboard shortcuts like cmd+t, cmd+l, return, escape, etc.",
        input_schema: .object(
            properties: [
                "key": .string("The key to press. Examples: 'a', 't', 'return', 'escape', 'tab', 'delete', 'up', 'down', 'left', 'right', 'space', 'f1'-'f12', '[', ']'"),
                "modifiers": .array("Optional: Array of modifier keys. Values: 'command', 'shift', 'option', 'control'. Example: ['command', 'shift']"),
            ],
            required: ["key"]
        )
    )
    
    static let openURL = ClaudeTool(
        name: "open_url",
        description: "Open a URL in the current browser app. This is faster than manually navigating for web browsers.",
        input_schema: .object(
            properties: [
                "url": .string("The URL to open (e.g., 'https://github.com' or just 'github.com')"),
            ],
            required: ["url"]
        )
    )
    
    static let wait = ClaudeTool(
        name: "wait",
        description: "Wait for a specified duration. Use this to wait for UI updates, page loads, or animations to complete.",
        input_schema: .object(
            properties: [
                "ms": .integer("Duration to wait in milliseconds (e.g., 500 for half a second, 1000 for one second)"),
            ],
            required: ["ms"]
        )
    )
    
    static let scroll = ClaudeTool(
        name: "scroll",
        description: "Scroll in the app. Use this to reveal elements that are off-screen.",
        input_schema: .object(
            properties: [
                "direction": .stringEnum("Scroll direction", values: ["up", "down", "left", "right"]),
                "amount": .integer("Scroll amount in pixels (default: 200)"),
                "element_id": .string("Optional: Element ID to scroll within"),
            ],
            required: ["direction"]
        )
    )
    
    static let focus = ClaudeTool(
        name: "focus",
        description: "Focus on a specific UI element. Use this to prepare for typing or other interactions.",
        input_schema: .object(
            properties: [
                "element_id": .string("The element ID to focus"),
            ],
            required: ["element_id"]
        )
    )
}
