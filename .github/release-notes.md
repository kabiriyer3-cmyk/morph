## Morph {{VERSION}} · Studio

Node-based multitool for your files: build a pipeline once, drop in thousands of files.

### What's new

- **Studio interface.** A strict graphite UI in the spirit of Figma: hairline borders instead of glow, Inter typography, a labeled tool rail, tabs and a floating dock with Add node and zoom. The accent color only marks selection and focus.
- **Dashboard.** Quick actions, 14 days of activity, recent runs, tool status and featured templates with graph previews. Files dropped on it go straight to Quick convert.
- **Boot animation.** Dots in 3D space gather into a sphere and morph into the Morph mark. Click to skip it, or turn it off in Settings. Settings also has a new *Start screen* option.
- **Nicer nodes.** Calmer node cards with a tinted icon chip, a node menu button, a progress line, hollow and filled sockets and thinner links.
- **Workflow overview.** With nothing selected the inspector shows nodes, links, categories, the last run and shortcuts.
- **New banner** with a 3D dot wave.

### Downloads

| Platform | File |
|---|---|
| Windows installer (recommended) | `Morph_{{VERSION}}_x64-setup.exe` |
| Windows MSI | `Morph_{{VERSION}}_x64_en-US.msi` |
| Windows portable (no install) | `Morph_{{VERSION}}_x64_portable.exe` |
| Linux AppImage | `Morph_{{VERSION}}_amd64.AppImage` |
| Debian / Ubuntu | `Morph_{{VERSION}}_amd64.deb` |
| Fedora / openSUSE | `Morph-{{VERSION}}-1.x86_64.rpm` |

Video and audio nodes need **FFmpeg**, which installs in one click from the **Tools** panel (or `winget install Gyan.FFmpeg`, `sudo apt install ffmpeg`). Office documents need **LibreOffice**, markup conversion needs **Pandoc**. Everything else (images, spreadsheets, 3D, archives, data, text, hashes, PDF basics) is built in.

Windows SmartScreen may warn because the build is not code-signed: click **More info → Run anyway**.
