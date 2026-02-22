import Foundation

/// App-specific hints for prompt engineering
enum AppHints {
    static func hints(for appName: String) -> String {
        let normalized = appName.lowercased()
        
        if normalized.contains("safari") {
            return safariHints
        } else if normalized.contains("messages") {
            return messagesHints
        } else if normalized.contains("finder") {
            return finderHints
        } else if normalized.contains("notes") {
            return notesHints
        } else if normalized.contains("mail") {
            return mailHints
        } else if normalized.contains("calendar") {
            return calendarHints
        } else if normalized.contains("terminal") {
            return terminalHints
        } else if normalized.contains("textedit") {
            return textEditHints
        } else if normalized.contains("preview") {
            return previewHints
        } else if normalized.contains("chrome") {
            return chromeHints
        } else if normalized.contains("firefox") {
            return firefoxHints
        } else if normalized.contains("slack") {
            return slackHints
        }
        
        return defaultHints
    }
    
    private static let safariHints = """
        Safari Browser Hints:
        - Keyboard shortcuts:
          • cmd+t: New tab
          • cmd+w: Close tab
          • cmd+l: Focus address bar (URL field)
          • cmd+r: Reload page
          • cmd+[: Go back
          • cmd+]: Go forward
          • cmd+shift+]: Next tab
          • cmd+shift+[: Previous tab
          • cmd+1-9: Switch to tab by number
        - To navigate to a URL: press cmd+l to focus address bar, type the URL, press return
        - The address bar is usually a TextField with role containing "address" or "URL"
        - Tab bar is at the top, each tab is a Button or Tab element
        - For clicking links, look for Link role elements
        - Use open_url tool for direct navigation when you have a specific URL
        """
    
    private static let messagesHints = """
        Messages App Hints:
        - Layout: Conversations list on left sidebar, messages on right, input at bottom
        - To send a message:
          1. Click on a conversation in the sidebar (look for StaticText with contact name)
          2. Click the message input field at the bottom (TextField or TextArea)
          3. Type your message
          4. Press return to send
        - Keyboard shortcuts:
          • cmd+n: New message
          • cmd+shift+n: New group message
          • return: Send message (when in input field)
        - The input field is usually the focused TextField at the bottom
        - Conversations may show as Row or Cell elements with contact names
        """
    
    private static let finderHints = """
        Finder Hints:
        - Layout: Sidebar on left with locations, main content area on right
        - Keyboard shortcuts:
          • cmd+shift+g: Go to Folder dialog
          • cmd+up: Go to parent folder
          • cmd+down: Open selected item
          • cmd+delete: Move to Trash
          • cmd+shift+delete: Empty Trash
          • cmd+1: Icon view
          • cmd+2: List view
          • cmd+3: Column view
          • cmd+n: New window
          • cmd+shift+n: New folder
          • space: Quick Look selected item
        - Sidebar items are usually Rows or StaticText with location names
        - Files and folders appear as Rows or Cells in the main area
        - To navigate: click sidebar location or use cmd+shift+g for path
        """
    
    private static let notesHints = """
        Notes App Hints:
        - Layout: Folders sidebar (left), Notes list (middle), Note content (right)
        - Keyboard shortcuts:
          • cmd+n: New note
          • cmd+shift+n: New folder
          • cmd+f: Find in notes
          • cmd+opt+f: Search all notes
        - To create a note:
          1. Click in the note content area or press cmd+n
          2. Start typing
        - Notes list shows as Table or List with Row elements
        - Note content is usually a TextArea or WebArea
        - Folders are in the left sidebar as OutlineRow or Row elements
        """
    
    private static let mailHints = """
        Mail App Hints:
        - Layout: Mailbox sidebar (left), Message list (middle), Message content (right)
        - Keyboard shortcuts:
          • cmd+n: New message
          • cmd+r: Reply
          • cmd+shift+r: Reply all
          • cmd+shift+f: Forward
          • cmd+shift+d: Send message
          • delete: Move to Trash
        - Compose window has To, Cc, Subject fields and a body TextArea
        - Mailboxes are in the left sidebar as Rows
        - Messages appear as Rows in the message list
        """
    
    private static let calendarHints = """
        Calendar App Hints:
        - Layout: Calendars sidebar (left), Calendar view (right)
        - Keyboard shortcuts:
          • cmd+n: New event
          • cmd+t: Go to today
          • cmd+1: Day view
          • cmd+2: Week view
          • cmd+3: Month view
          • cmd+4: Year view
        - To create an event: cmd+n or double-click on a day/time
        - Events appear as Buttons or StaticText in the calendar grid
        - Calendar names are in the sidebar as CheckBoxes
        """
    
    private static let terminalHints = """
        Terminal App Hints:
        - The main content is a TextArea representing the terminal session
        - Keyboard shortcuts:
          • cmd+n: New window
          • cmd+t: New tab
          • cmd+w: Close tab
          • cmd+k: Clear screen
        - To type commands: click in the terminal area and type
        - The terminal content is usually a single large TextArea or Group
        - Output is typically not individually selectable as UI elements
        """
    
    private static let textEditHints = """
        TextEdit Hints:
        - Simple text editor with a main TextArea for content
        - Keyboard shortcuts:
          • cmd+n: New document
          • cmd+s: Save
          • cmd+shift+s: Save As
          • cmd+o: Open
        - The document content is a TextArea
        - Format bar at top (if in Rich Text mode)
        """
    
    private static let previewHints = """
        Preview App Hints:
        - Shows images and PDFs
        - Keyboard shortcuts:
          • cmd+plus: Zoom in
          • cmd+minus: Zoom out
          • cmd+0: Actual size
          • left/right arrows: Previous/next page (PDFs)
        - Sidebar shows thumbnails for multi-page documents
        - Main content area displays the current page/image
        """
    
    private static let chromeHints = """
        Google Chrome Hints:
        - Similar to Safari for basic navigation
        - Keyboard shortcuts:
          • cmd+t: New tab
          • cmd+w: Close tab
          • cmd+l: Focus address bar
          • cmd+r: Reload
          • cmd+shift+t: Reopen closed tab
        - Address bar is called "omnibox" internally
        - Tabs are at the top as Tab elements
        """
    
    private static let firefoxHints = """
        Firefox Hints:
        - Similar to Safari for basic navigation
        - Keyboard shortcuts:
          • cmd+t: New tab
          • cmd+w: Close tab
          • cmd+l: Focus address bar
          • cmd+r: Reload
        - Address bar is a TextField/ComboBox at the top
        - Tabs are at the top as Tab elements
        """
    
    private static let slackHints = """
        Slack Hints:
        - Layout: Workspace sidebar (left), Channel list, Messages (right)
        - Keyboard shortcuts:
          • cmd+k: Quick switcher (search channels/DMs)
          • cmd+n: New message
          • cmd+/: Keyboard shortcuts help
        - To send a message:
          1. Click on a channel or DM in the sidebar
          2. Click the message input at the bottom (TextArea)
          3. Type and press return to send
        - Channels are in the sidebar as Rows or StaticText
        """
    
    private static let defaultHints = """
        General macOS App Hints:
        - Most apps have a menu bar at the top with application menus
        - Common keyboard shortcuts:
          • cmd+q: Quit
          • cmd+w: Close window
          • cmd+n: New (document/window)
          • cmd+o: Open
          • cmd+s: Save
          • cmd+z: Undo
          • cmd+c/v/x: Copy/Paste/Cut
          • cmd+a: Select all
          • cmd+f: Find
          • cmd+,: Preferences
        - Look for common UI patterns:
          • Toolbar at top
          • Sidebar on left
          • Main content in center
          • Status bar at bottom
        - Start with observe_ui to understand the layout
        - Use keyboard shortcuts when available (faster and more reliable)
        """
}
