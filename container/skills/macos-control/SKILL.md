---
name: macOS Control
description: Control macOS system surfaces using private and public framework APIs. Discover APIs from the DYLD shared cache, generate code, compile and execute on the host.
allowed-tools: discover_framework, query_apis, swift_interface, execute_code, observe_ui, perform_action, kill_background, list_frameworks
---

# macOS Control

You can discover and call any macOS framework API — including PrivateFrameworks — by using these MCP tools.

## Workflow

### 1. Discover what's available

```
discover_framework("CoreDock")   → C/ObjC exports + Swift interface
swift_interface("SkyLight")      → complete Swift type declarations
query_apis("window compositing") → search across all cached frameworks
```

### 2. Write code that uses the APIs

```
execute_code({
  code: "import AppKit\nlet item = NSStatusBar.system.statusItem(withLength: .variableLength)\nitem.button?.title = \"Hello\"\nRunLoop.main.run()",
  frameworks: ["AppKit"],
  background: true
})
```

### 3. Verify with accessibility

```
observe_ui("SystemUIServer")  → check if your status item appeared
```

### 4. Iterate

If something doesn't work, check the error, adjust the code, try again.

## Key Frameworks

| Goal                | Framework                      | Notes                                                                   |
| ------------------- | ------------------------------ | ----------------------------------------------------------------------- |
| Menu bar items      | `AppKit` (NSStatusBar)         | Public API, reliable                                                    |
| Dock manipulation   | `CoreDock`                     | Private. Functions like `CoreDockGetItemCount`, `CoreDockCopyItemTitle` |
| Window compositing  | `SkyLight`                     | Private. Controls window server                                         |
| Widget surfaces     | `ChronoCore`, `ChronoServices` | Private. Widget lifecycle                                               |
| Cross-process views | `ViewBridge`                   | Private. NSView hosting across processes                                |
| GPU/Metal internals | `MTLCompiler`, `IOGPU`         | Private. GPU pipeline access                                            |
| System UI           | `SystemUIPlugin`               | Private. SystemUIServer plugins                                         |

## Swift Code Patterns

### Persistent menu bar item (background process)

```swift
import AppKit

let app = NSApplication.shared
app.setActivationPolicy(.accessory) // No dock icon

let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
statusItem.button?.title = "🔥 NanoClaw"

let menu = NSMenu()
menu.addItem(NSMenuItem(title: "Status: Active", action: nil, keyEquivalent: ""))
menu.addItem(NSMenuItem.separator())
menu.addItem(NSMenuItem(title: "Quit", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))
statusItem.menu = menu

app.run()
```

### Real-time updating UI

```swift
import AppKit

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

Timer.scheduledTimer(withTimeInterval: 1.0, repeats: true) { _ in
    let fmt = DateFormatter()
    fmt.dateFormat = "HH:mm:ss"
    statusItem.button?.title = "⏱ \(fmt.string(from: Date()))"
}
RunLoop.main.run()
```

### Using a private framework via dlopen

```swift
import Foundation

typealias CoreDockGetItemCount = @convention(c) () -> Int32
let handle = dlopen("/System/Library/PrivateFrameworks/CoreDock.framework/CoreDock", RTLD_NOW)
if let sym = dlsym(handle, "CoreDockGetItemCount") {
    let fn = unsafeBitCast(sym, to: CoreDockGetItemCount.self)
    print("Dock items: \(fn())")
}
```

### Using Python ctypes for quick probing

```python
import ctypes
lib = ctypes.CDLL("/System/Library/PrivateFrameworks/CoreDock.framework/CoreDock")
lib.CoreDockGetItemCount.restype = ctypes.c_int
print(f"Dock items: {lib.CoreDockGetItemCount()}")
```

## Rules

- Always call your code from your own process space — don't inject into system processes
- Use `background: true` for persistent UI (menu bar, dock tiles, timers)
- Use `kill_background` to clean up when done
- Check `observe_ui` to verify visual changes
- Private APIs may change between OS versions — discover dynamically, don't hardcode
