# Scope 🔭

A beautiful terminal user interface (TUI) for managing Linux packages across multiple package managers.

![Rust](https://img.shields.io/badge/Rust-1.70+-orange?style=flat-square&logo=rust)
![License](https://img.shields.io/badge/License-MIT-blue?style=flat-square)
![Platform](https://img.shields.io/badge/Platform-Linux-green?style=flat-square&logo=linux)

## ✨ Features

- **Multi-Package Manager Support** - Manage packages from:
  - 📦 **APT** (Debian/Ubuntu)
  - 🔶 **Snap**
  - 📱 **Flatpak**
  - 🖼️ **AppImage**

- **Beautiful TUI** - Modern terminal interface with:
  - Retro Warmth color theme (Gruvbox-inspired)
  - Real-time package scanning with streaming updates
  - Responsive keyboard navigation

- **Powerful Package Management**:
  - 🔍 **Real-time search** - Type to filter packages instantly
  - 📊 **Multiple sort options** - By size, name, or source
  - 🏷️ **Filter by type** - GUI apps, CLI tools, or all
  - 🗑️ **Uninstall packages** - With confirmation dialog
  - 🔄 **Check for updates** - Batch update support

## 📸 Screenshot

```
┌─ SCOPE ─────────────────────────────────────────────────────────────────────┐
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │ > Apps         │ All │ APT │ Snap │ Flatpak │ AppImage │  172 pkgs     ││
│  │   Updates      ├─────────────────────────────────────────────────────────┤│
│  │   Clean        │ Packages (163/172) - Sort: Size (largest first)        ││
│  │                │─────────────────────────────────────────────────────────││
│  │                │   Name                 Source    Type   Size           ││
│  │                │ > antigravity          apt       GUI    710.13 MiB     ││
│  │                │   obsidian             flatpak   GUI    636.70 MiB     ││
│  │                │   cursor               apt       GUI    582.91 MiB     ││
│  │                │   google-chrome        apt       GUI    378.72 MiB     ││
│  │                │   docker.io            apt       CLI    104.38 MiB     ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │ Search...                    │ [Tab] Source │ [d] Del │ [s] Sort       ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

## 🚀 Installation

### Prerequisites

- Rust 1.70 or higher
- Linux operating system
- Package managers you want to manage (apt, snap, flatpak, etc.)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/scope.git
cd scope

# Build release version
cargo build --release

# Run the application
./target/release/scope
```

### Install Globally

```bash
# Install to ~/.cargo/bin
cargo install --path .

# Or copy to /usr/local/bin
sudo cp ./target/release/scope /usr/local/bin/
```

## ⌨️ Keyboard Shortcuts

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `←` / `h` | Focus sidebar |
| `→` / `l` | Exit sidebar |
| `Enter` | Select / View details |
| `Tab` | Next source filter |
| `Shift+Tab` | Previous source filter |
| `Home` / `g` | Jump to first |
| `End` / `G` | Jump to last |
| `PageUp` / `PageDown` | Page navigation |

### Actions

| Key | Action |
|-----|--------|
| `d` | Delete/Uninstall package |
| `s` | Toggle sort mode |
| `f` | Toggle filter (All/GUI/CLI) |
| `r` | Refresh package list |
| `Esc` | Clear search / Go back / Quit |
| `q` | Quit application |

### Search

Just start typing to filter packages in real-time. Press `Esc` to clear the search.

### Updates Section

Navigate to the **Updates** section in the sidebar and press `Enter` to:
1. Check all packages for available updates
2. Select which packages to update
3. Batch update selected packages

## 🎨 Color Theme

Scope uses a **Retro Warmth** color palette inspired by Gruvbox:

| Element | Color | Purpose |
|---------|-------|---------|
| Background | `#1d2021` | Soft dark background |
| Primary Text | `#ebdbb2` | Warm cream for main content |
| Secondary Text | `#d5c4a1` | Muted beige for metadata |
| Borders | `#b8bb26` | Yellow-green accents |
| CLI Indicator | `#fe8019` | Orange for CLI apps |
| Warnings/Errors | `#fb4934` | Red for alerts |

## 📁 Project Structure

```
scope/
├── Cargo.toml          # Project configuration
├── src/
│   ├── main.rs         # Entry point and event handling
│   ├── app.rs          # Application state management
│   ├── package.rs      # Package data structures
│   ├── theme.rs        # Color theme definitions
│   ├── scanner/        # Package manager scanners
│   │   ├── mod.rs      # Scanner coordinator
│   │   ├── apt.rs      # APT scanner
│   │   ├── snap.rs     # Snap scanner
│   │   ├── flatpak.rs  # Flatpak scanner
│   │   └── appimage.rs # AppImage scanner
│   └── ui/             # User interface components
│       ├── mod.rs      # UI coordinator
│       ├── main_view.rs    # Package list view
│       ├── sidebar.rs      # Navigation sidebar
│       ├── details_view.rs # Package details
│       └── dialogs.rs      # Confirmation dialogs
└── README.md
```

## 🔧 Dependencies

- **[ratatui](https://github.com/ratatui-org/ratatui)** - Terminal UI framework
- **[crossterm](https://github.com/crossterm-rs/crossterm)** - Terminal manipulation
- **[tokio](https://tokio.rs/)** - Async runtime
- **[serde](https://serde.rs/)** - Serialization
- **[fuzzy-matcher](https://crates.io/crates/fuzzy-matcher)** - Fuzzy search
- **[humansize](https://crates.io/crates/humansize)** - Human-readable sizes

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by tools like `htop`, `lazygit`, and `ncdu`
- Color palette based on [Gruvbox](https://github.com/morhetz/gruvbox)
- Built with the amazing [Ratatui](https://ratatui.rs/) library

---

Made with ❤️ and Rust 🦀
