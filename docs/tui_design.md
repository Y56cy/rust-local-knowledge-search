# TUI Design

A terminal user interface can provide interactive search, keyboard navigation, document preview, bookmarks, search history and statistics. This project uses ratatui and crossterm.

On Windows, keyboard events should only process KeyEventKind::Press, otherwise one typed character may be handled twice.
