First release of the Rust cross-platform rewrite.

Aurora Asset Editor now runs on Windows, macOS and Linux, with nothing else to install. Linux needs Ubuntu 22.04, Debian 12 or newer. For the same image, the `.asset` files it writes are byte for byte what 1.x writes. Resized and JPEG images can differ from 1.x by a few shades, because image decoding and resizing no longer go through Windows.

## Changes from 1.x

- New layout: pages in a sidebar, Open and Save menus, a current-game chip and a status line. Progress shows in the status line instead of a busy overlay. Light and dark follow the system, with a choice under the gear.
- More image formats: WebP, DDS, TGA and QOI now open everywhere an image is accepted, along with TIFF and ICO, and images can be saved as WebP.
- Auto Resize, Use Compression, the theme and the window size and position are remembered. Existing 1.x FTP settings carry over on Windows.
- Toasts confirm saves, exports, copies to the clipboard and transfers to or from the console. Errors also show as red toasts.
- Unsaved changes are no longer lost by accident. When a page has unsaved changes, opening files, Open in editor, downloading from the console, double-clicking a console game, New asset set and closing the window (Cmd+Q on macOS included) first ask "Discard unsaved changes to ...?", naming those pages. Cancel is the default.
- An image that fails to load leaves the existing one in place. Dropping several screenshots fills the free slots.
- Screenshots can be reordered by dragging. Moving or packing screenshots keeps their stored image instead of re-encoding it.
- Console: Open in editor and Download all replace every asset type with what the console has, and change nothing when any part fails.
- Console: double-click a game, or press Return, to open it in the editor.
- Console: Save all to console uploads straight away when a game is selected. With no console set up, it is disabled and says why.
- Console: Remove asks first and no longer clears the editor. The game list shows which asset files each game has. One Refresh button instead of two.
- Console: the IP address, port, username and password are set in a Console connection dialog. The password shows as plain text, as in 1.x.
- The TitleID and DatabaseID dialogs offer a filterable list of the console's games. Typing the IDs still works.
- Bulk covers: the dialog says what the run will do, the connection is checked first, and a failed game no longer stops the run. A run can be stopped. A summary counts each outcome, including the games a stopped run did not handle.
- Online: an empty TitleID is refused instead of searching for 00000000, and a failed download stops instead of retrying forever. XboxUnity results show the rating on its own line.
- Missing files and folders on the command line are reported instead of being skipped.
- Fixes for FSD files and for console transfers that could hang or retry forever.
- The error log moved to the app's settings folder. About links to it and can copy the build information. `--version` prints it.
- Removed: the Xbox.com source, its locale list and "Cover Art Only?", because those services are gone. Bulk actions fetch covers only.
