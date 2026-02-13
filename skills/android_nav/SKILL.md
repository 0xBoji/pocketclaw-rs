---
name: android_nav
description: Interact with an Android device to navigate apps and perform actions.
---

# Android Navigation

You have the ability to control an Android device using the `android_screen` and `android_action` tools.
Use this skill when the user asks you to perform actions on their phone, such as opening apps, searching, or typing.

## Tools

### 1. `android_screen`
- **Action**: `dump_hierarchy`
- **Purpose**: "See" the current screen. Returns an XML/JSON-like tree of UI elements.
- **Usage**: Call this FIRST to find the coordinates or IDs of elements you want to interact with.
- **Action**: `screenshot`
- **Purpose**: Capture a visual screenshot (returns bytes). useful for debugging or saving images.

### 2. `android_action`
- **Action**: `click`
    - **Args**: `x` (float), `y` (float)
    - **Purpose**: Tap on a specific point on the screen.
- **Action**: `input_text`
    - **Args**: `text` (string)
    - **Purpose**: Type text into the currently focused input field.
    - **Tip**: Click the input field first to ensure it's focused!
- **Action**: `scroll`
    - **Args**: `x1`, `y1`, `x2`, `y2`
    - **Purpose**: Swipe from (x1, y1) to (x2, y2).
- **Action**: `home`
    - **Purpose**: Go to the home screen.
- **Action**: `back`
    - **Purpose**: Go back.

## Workflow: "Open App and Search"

1.  **Go Home**: logic might be deep in an app, so start by pressing Home.
    -   `android_action(action="home")`
2.  **Find App**: Look for the app icon.
    -   `android_screen(action="dump_hierarchy")`
    -   Analyze the tree to find `text="Facebook"` or content-desc.
    -   If not found, try scrolling or ask user.
3.  **Open App**: Click the icon.
    -   `android_action(action="click", x=..., y=...)`
4.  **Wait**: Apps take time to load. You might need to wait a second (or just retry dump).
5.  **Find Search**: Look for the search button/bar.
    -   `android_screen(action="dump_hierarchy")`
    -   Find `content-desc="Search"` or similar.
6.  **Click Search**:
    -   `android_action(action="click", x=..., y=...)`
7.  **Type Query**:
    -   `android_action(action="input_text", text="OpenClaw VN")`
8.  **Submit**:
    -   Often need to click a "Search" or "Enter" button on the keyboard or UI.
    -   `android_screen(action="dump_hierarchy")` -> Find submit button -> Click.

## Tips
- Always `dump_hierarchy` before clicking to ensure you have up-to-date coordinates.
- If you fail to find an element, try `scroll` to see more content.
- `input_text` only works if an editable field is focused.
