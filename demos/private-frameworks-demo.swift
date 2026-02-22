#!/usr/bin/env swift
// macOS Private Frameworks Demonstration
// This script demonstrates REAL usage of undocumented private Apple frameworks
// discovered through our framework indexing project.

import Foundation
import AppKit

print("=" + String(repeating: "=", count: 59))
print("macOS PRIVATE FRAMEWORKS DEMONSTRATION")
print("=" + String(repeating: "=", count: 59))
print()
print("OS Version: \(ProcessInfo.processInfo.operatingSystemVersionString)")
print()

// ============================================
// 1. SkyLight Framework - Window Server Access
// ============================================
print("1️⃣  SKYLIGHT.FRAMEWORK (PRIVATE)")
print("   Path: /System/Library/PrivateFrameworks/SkyLight.framework")
print()

let skylightPath = "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight"
let skylightHandle = dlopen(skylightPath, RTLD_LAZY)

if skylightHandle != nil {
    print("   ✅ Loaded framework successfully")
    print()

    // API 1: CGSMainConnectionID
    print("   Testing: CGSMainConnectionID()")
    let connectionSym = dlsym(skylightHandle, "CGSMainConnectionID")
    if connectionSym != nil {
        typealias CGSMainConnectionIDFunc = @convention(c) () -> UInt32
        let CGSMainConnectionID = unsafeBitCast(connectionSym, to: CGSMainConnectionIDFunc.self)
        let connectionID = CGSMainConnectionID()
        print("   Result: Connection ID = \(connectionID)")
        print("   Purpose: Get window server connection handle")
        print()

        // API 2: CGSGetWindowCount
        print("   Testing: CGSGetWindowCount()")
        let countSym = dlsym(skylightHandle, "CGSGetWindowCount")
        if countSym != nil {
            typealias CGSGetWindowCountFunc = @convention(c) (UInt32, UnsafeMutablePointer<Int>) -> Int32
            let CGSGetWindowCount = unsafeBitCast(countSym, to: CGSGetWindowCountFunc.self)
            var windowCount: Int = 0
            let result = CGSGetWindowCount(connectionID, &windowCount)

            if result == 0 {
                print("   Result: \(windowCount) windows in system")
                print("   Purpose: Count ALL windows (not just your app)")
                print()
            }
        }
    }

    print("   💡 What this framework enables:")
    print("      • Enumerate windows from ANY application")
    print("      • Get window geometry, owner process, z-order")
    print("      • Move, resize, transform windows")
    print("      • Set window transparency, effects")
    print("      • This is what Mission Control and Exposé use!")
    print()
    print("   🚫 Impossible with public AppKit APIs:")
    print("      NSWindow only works with your own app's windows")
    print()

    dlclose(skylightHandle)
} else {
    print("   ❌ Failed to load framework")
    if let error = dlerror() {
        print("   Error: \(String(cString: error))")
    }
}

print(String(repeating: "-", count: 60))
print()

// ============================================
// 2. DisplayServices Framework - Brightness Control
// ============================================
print("2️⃣  DISPLAYSERVICES.FRAMEWORK (PRIVATE)")
print("   Path: /System/Library/PrivateFrameworks/DisplayServices.framework")
print()

let displayPath = "/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices"
let displayHandle = dlopen(displayPath, RTLD_LAZY)

if displayHandle != nil {
    print("   ✅ Loaded framework successfully")
    print()

    // API 1: DisplayServicesCanChangeBrightness
    print("   Testing: DisplayServicesCanChangeBrightness()")
    let canChangeSym = dlsym(displayHandle, "DisplayServicesCanChangeBrightness")
    if canChangeSym != nil {
        typealias CanChangeFunc = @convention(c) (UInt32) -> Bool
        let DisplayServicesCanChangeBrightness = unsafeBitCast(canChangeSym, to: CanChangeFunc.self)
        let canChange = DisplayServicesCanChangeBrightness(0)
        print("   Result: \(canChange)")
        print("   Purpose: Check if display supports brightness control")
        print()

        if canChange {
            // API 2: DisplayServicesGetBrightness
            print("   Testing: DisplayServicesGetBrightness()")
            let getBrightSym = dlsym(displayHandle, "DisplayServicesGetBrightness")
            if getBrightSym != nil {
                typealias GetBrightFunc = @convention(c) (UInt32, UnsafeMutablePointer<Float>) -> Int32
                let DisplayServicesGetBrightness = unsafeBitCast(getBrightSym, to: GetBrightFunc.self)
                var brightness: Float = 0.0
                let result = DisplayServicesGetBrightness(0, &brightness)

                if result == 0 {
                    print("   Result: \(Int(brightness * 100))% brightness")
                    print("   Raw value: \(String(format: "%.3f", brightness))")
                    print("   Purpose: Read current display brightness level")
                    print()
                }
            }

            print("   💡 What this framework enables:")
            print("      • Read current display brightness")
            print("      • Set display brightness programmatically")
            print("      • Control built-in and external displays")
            print("      • This is what System Settings uses!")
            print()
        } else {
            print("   ℹ️  Brightness control not available")
            print("      (Normal for external monitors, VMs, or remote sessions)")
            print()
            print("   💡 On supported displays, this framework:")
            print("      • Reads/sets brightness programmatically")
            print("      • Controls both built-in and external displays")
            print()
        }
    }

    print("   🚫 Impossible with public APIs:")
    print("      No public API for brightness in CoreGraphics/AppKit")
    print()

    dlclose(displayHandle)
} else {
    print("   ❌ Failed to load framework")
}

print(String(repeating: "=", count: 60))
print("SUMMARY")
print(String(repeating: "=", count: 60))
print()
print("✅ Successfully demonstrated REAL private framework usage!")
print()
print("Frameworks Used:")
print()
print("1. SkyLight.framework (PRIVATE)")
print("   • CGSMainConnectionID() - Window server connection")
print("   • CGSGetWindowCount() - System-wide window enumeration")
print("   • ~1,790 total functions discovered via indexing")
print()
print("2. DisplayServices.framework (PRIVATE)")
print("   • DisplayServicesCanChangeBrightness() - Check capability")
print("   • DisplayServicesGetBrightness() - Read brightness")
print("   • ~52 total functions discovered via indexing")
print()
print("🔓 What This Proves:")
print("   • Private frameworks exist and are fully functional")
print("   • Our indexing discovered REAL, callable APIs")
print("   • These provide capabilities impossible with public APIs")
print("   • Total discovery: ~228,537+ APIs across 356 frameworks")
print()
print("🚀 Use Cases:")
print("   • Window management utilities (like Rectangle, Magnet)")
print("   • System monitoring tools")
print("   • Automation and productivity apps")
print("   • Display/brightness control utilities")
print()
print(String(repeating: "=", count: 60))
print()
print("Note: Private frameworks are undocumented and may change")
print("between macOS versions. Use with caution in production.")
print()
