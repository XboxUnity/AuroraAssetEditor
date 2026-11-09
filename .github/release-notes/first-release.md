First release of the Rust cross-platform rewrite.

Aurora Asset Editor now runs on Windows, macOS and Linux, as one file per system with nothing else to install. The `.asset` files it writes are byte for byte what 1.x writes for the same image and settings.

## Changes from 1.x

- New layout: pages in a sidebar, Open and Save menus, a current-game chip and a status line. Light and dark follow the system, with a choice under the gear.
- More image formats: WebP, TIFF, TGA, ICO and QOI open everywhere an image is accepted, and images can be saved as WebP.
- Auto Resize, Use Compression, the theme and the window size and position are remembered.
- Toasts confirm saves, exports, copies to the clipboard and transfers to or from the console; errors also show as red toasts.
- Unsaved changes are no longer lost by accident. Opening files, Open in editor, downloading from the console (Download all or one kind), double-clicking a console game, New asset set and closing the window (Cmd+Q on macOS included) first ask "Discard unsaved changes to ...?", naming the changed pages, with Discard and Cancel. Cancel is the default.
- Console: Open in editor and Download all replace every asset type with what the console has, and change nothing when any part fails.
- Console: double-click a game, or press Return, to open it in the editor.
- Console: Save all to console uploads straight away when a game is selected.
- Console: Remove asks first and no longer clears the editor. The game list shows which asset files each game has.
- Console: the IP address, port, username and password are set in a Console connection dialog. The password shows as plain text, as in 1.x.
- The TitleID and DatabaseID dialogs offer a filterable list of the console's games; typing the IDs still works.
- Bulk covers: the dialog says what the run will do, the connection is checked first, and a failed game no longer stops the run. A summary counts each outcome, including the games a stopped run did not handle.
- Screenshots can be reordered by dragging. Moving or packing screenshots keeps their stored image instead of re-encoding it.
- Online: an empty TitleID is refused instead of searching for 00000000, and a failed download stops instead of retrying forever. XboxUnity results show the rating on its own line.
- Files on the command line that are not asset files are reported instead of being opened as FSD files.
- The error log moved to the app's settings folder; About links to it.
- Removed: the Xbox.com source, its locale list and "Cover Art Only?", because those services are gone. Bulk actions fetch covers only.
