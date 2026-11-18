# Copy and paste

## Copy from a remote program to your clipboard

Programs on the server copy with OSC 52: they print the text in a special sequence, and the terminal puts it on the clipboard. lazyssh catches those sequences in a tab and passes them on to your real terminal.

- **Neovim:** yank to the `+` register, for example `"+y`, or a mapping like `<leader>y` for `"+y`. Neovim 0.10 and newer use OSC 52 on their own when they see the terminal supports it, and lazyssh says it does. Nothing to set up on the server.
- **tmux, other editors:** anything that copies with OSC 52 works the same way.

Your own terminal has to allow programs to set the clipboard. Ghostty, Kitty, WezTerm, and Alacritty do by default.

Copies up to about 750 KB of text get through. Reading the clipboard from the server (OSC 52 with `?`, which is `"+p` in Neovim) isn't passed on. Paste with your terminal instead.

## Paste into a session

Paste with your terminal's paste key, for example `Ctrl+Shift+V`. lazyssh sends the text to the server.

If the program on the server asked for bracketed paste, as Neovim and most shells do, lazyssh marks the text as a paste. Neovim then inserts it as-is, without indenting every line as if you typed it.

## Paste into lazyssh

In the filter and the add or edit form, a paste is typed into the field. Line breaks are dropped, so a paste can't submit the form halfway. In the host list itself a paste is ignored, so a `q` or `d` in the text can't quit or delete anything.

## Copy text from the screen

lazyssh uses the mouse, so dragging normally selects inside lazyssh. Hold `Shift` while you drag to make your terminal select instead, then copy with your terminal's copy key, for example `Ctrl+Shift+C`.
