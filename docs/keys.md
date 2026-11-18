# Keys

Press `?` in lazyssh to see these.

## In the host list

| Key | What it does |
|---|---|
| `↑` `↓` or `k` `j` | Move up and down |
| `g` / `G` | First / last host |
| `/` | Filter the list as you type |
| `Esc` | Clear the filter. With no filter, quit |
| `Enter` | Connect to the host |
| `n` | Next tab |
| `w` | Close the tab |
| `a` | Add a host |
| `e` | Edit the host |
| `d` | Delete the host. Asks first |
| `r` | Read `~/.ssh/config` again |
| `c` | Show the `ssh` command for the host |
| `s` | Settings |
| `t` | Themes |
| `T` | Transparency on or off |
| `?` | Help |
| `q` or `Ctrl-C` | Quit |

## In a session

While a tab has focus, every key goes to the server, so `q`, `w`, and the rest work as they would in a normal terminal. Only one key is lazyssh's:

| Key | What it does |
|---|---|
| `Ctrl-B` | Show or hide the host list. Showing it moves focus to the list; hiding it moves focus back to the session |

Moving the mouse over the host list or a session moves focus there too.

## Mouse

- Scroll over the host list to move through it.
- In a session, clicks, drags, and the wheel go to the program on the server when it turned the mouse on. In Neovim that means a click moves the cursor, a drag selects, and the wheel scrolls the buffer. The same goes for htop, `less --mouse`, and tmux with `mouse on`.
- When the program didn't turn the mouse on, like a plain shell prompt, the wheel scrolls back through the session's earlier output instead.
- Double-click a host to connect.
- Click a tab to switch to it, or its close button to close it.
- Hold `Shift` while dragging to select text with your terminal instead of lazyssh, then copy it with your terminal's copy key.
