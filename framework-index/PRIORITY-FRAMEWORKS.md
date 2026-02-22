# High-Priority Frameworks for Reverse Engineering

**Created:** 2026-02-22
**Strategy:** Focus on private frameworks with substantial implementation over public API wrappers

## Prioritization Criteria

1. **Private > Public** - Private frameworks contain actual implementation, not just API surface
2. **High Function Count** - More functions = more capabilities to discover
3. **System Integration** - Frameworks that control core macOS functionality
4. **Undocumented** - No official Apple documentation
5. **Powerful Capabilities** - Access to restricted/privileged operations

## Top Priority: Private System Frameworks

### Tier 1: Critical System Services (MUST INDEX)
These control core macOS functionality and are completely undocumented:

- **IMCore** ✅ DONE (2,568 functions) - Full iMessage internals
- **SkyLight** ✅ DONE (1,790 functions) - Window compositing, Mission Control
- **MediaRemote** ✅ DONE (2,237 functions) - Media playback control
- **TCC** ✅ DONE (206 functions) - Privacy permissions database
- **CoreDock** - Dock internals (NEED TO RETRY - failed earlier)
- **SpringBoard** - iOS/macOS launcher (may be iOS-only)
- **FrontBoard** ✅ DONE (82 functions) - App lifecycle
- **RunningBoard** ✅ DONE (33 functions) - Process management
- **BackBoardServices** ✅ DONE (134 functions) - HID events

### Tier 2: Communication & Messaging
- **Messages** ✅ DONE (1 Swift type) - Messages framework
- **MessageUI** ✅ DONE (4 Swift types) - Message composition
- **IMCore** ✅ DONE - iMessage backend
- **FaceTime** - FaceTime internals (need to index)
- **TelephonyUtilities** - Phone/call internals (need to index)
- **CoreTelephony** ✅ DONE (631 functions)

### Tier 3: System Control & Automation
- **SystemStatus** ✅ DONE (127 functions) - Menu bar, system indicators
- **StatusKit** ✅ DONE (7 functions) - Status indicators
- **CoreAnalytics** ✅ DONE (91 functions) - Analytics tracking
- **Install** ✅ DONE (437 functions) - macOS installer internals
- **MobileDevice** ✅ DONE (1,253 functions) - iOS device communication
- **DiskImages** ✅ DONE (274 functions) - DMG operations

### Tier 4: Authentication & Security
- **Heimdal** ✅ DONE (1,250 functions) - Kerberos authentication
- **SecurityFoundation** ✅ DONE (142 functions) - Security utilities
- **Security** ✅ DONE (3,050 functions) - Keychain, certificates
- **LocalAuthentication** ✅ DONE (83 functions) - Touch ID, Face ID
- **AuthenticationServices** ✅ DONE (1,575 functions) - Passkeys

### Tier 5: Media & Graphics
- **AudioToolboxCore** ✅ DONE (1,303 functions) - Audio DSP
- **MediaToolbox** ✅ DONE (2,989 functions) - Media processing
- **CoreMediaIO** ✅ DONE (231 functions) - Camera/device I/O
- **VideoToolbox** ✅ DONE (457 functions) - Video encoding
- **QuartzCore** ✅ DONE (928 functions) - Core Animation

### Tier 6: Machine Learning & Recognition
- **CorePrediction** ✅ DONE (78 functions) - ML models
- **CoreRecognition** ✅ DONE (129 functions) - OCR, text recognition
- **Vision** ✅ DONE (6,349 functions) - Computer vision
- **CoreML** ✅ DONE (exceeded token limit) - ML framework

## Lower Priority: Public Frameworks

These are well-documented and mostly wrappers around private frameworks:

- **SwiftUI** ✅ DONE (14,312 functions) - Public UI framework
- **UIKit** - iOS UI framework (may not be on macOS)
- **AppKit** ✅ DONE (6,068 functions) - macOS UI (mostly documented)
- **Foundation** ✅ DONE (11,371 functions) - Core framework (well-documented)
- **Combine** ✅ DONE (1,544 functions) - Reactive programming (public)
- **WidgetKit** ✅ DONE (3,282 functions) - Widgets (public)

## Frameworks to Index Next

### Immediate Priority (Next 10)
1. **CoreDock** - Dock internals (RETRY)
2. **FaceTime** - FaceTime framework
3. **TelephonyUtilities** - Phone/call internals
4. **PreferencePanes** ✅ DONE (26 functions) - System Preferences
5. **LaunchServices** ✅ DONE (Swift-only) - App launching
6. **CoreSymbolication** ✅ DONE (443 functions) - Symbol resolution
7. **IOKit** ✅ DONE (2,247 functions) - Hardware I/O
8. **CoreWLAN** ✅ DONE (340 functions) - WiFi
9. **SystemConfiguration** ✅ DONE (574 functions) - Network config
10. **DiskArbitration** ✅ DONE (70 functions) - Disk events

### Private Frameworks to Prioritize
Look for frameworks in `/System/Library/PrivateFrameworks/` with:
- High function counts
- System integration capabilities
- Undocumented features

### Skip These (Low Value)
- Playground frameworks (iOS/iPadOS only)
- Deprecated frameworks (SyncServices, DVDPlayback)
- Umbrella frameworks with 0 functions
- iOS-only frameworks that error with "undefined"

## Strategy Going Forward

1. **Phase 1**: Index all Tier 1-3 private frameworks FIRST
2. **Phase 2**: Index high-value private frameworks from /System/Library/PrivateFrameworks/
3. **Phase 3**: Fill in remaining public frameworks for completeness
4. **Phase 4**: Retry failed frameworks (CoreDock, etc.)

## Notes

- Messages.framework itself is Swift-only (1 type), but IMCore has the actual implementation
- Many "frameworks" are just wrappers - look for high function counts
- Private frameworks in /System/Library/PrivateFrameworks/ are the goldmine
- Public frameworks in /System/Library/Frameworks/ are mostly well-documented

---

**Bottom Line:** Focus on private frameworks with 100+ functions that control system functionality.
