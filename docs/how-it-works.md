# How it works

lazyssh is a list of the hosts in your `~/.ssh/config` with a terminal next to it. It doesn't keep its own list of hosts: what you see is what's in the file, and what you add or change is written back to it.

```
~/.ssh/config → lazyssh → ssh <alias> → the server
```

## The hosts file

- lazyssh reads `~/.ssh/config` when it starts. Press `r` to read it again after you edit it somewhere else.
- Adding, editing, or deleting a host writes the whole file back. Before every write it copies the old file to `~/.ssh/config.backup_<date>_<time>`, so a mistake can always be undone by copying a backup back.
- Each host has an alias, a HostName, a port, a user, an IdentityFile, and any other options you add. Other options in a host block, like `ProxyJump`, are kept.

### What a save doesn't keep

Everything above the first `Host` line is written back exactly as it was. After that, a save loses:

- `Host *` blocks, since lazyssh skips them when it reads the file
- Comments inside a host block, and comments at the end of a line

Keep `Host *` settings and long comments above your first `Host` line, or restore them from the backup after a save.

## Reachability

In the background, lazyssh tries to open a connection to each host's SSH port, eight hosts at a time, with a 2 second timeout. The dot next to each host shows the result:

| Dot | Means |
|---|---|
| green `●` | The SSH port answered |
| red `●` | Nothing answered within 2 seconds |
| `◌` | Still checking |
| `○` | Not checked yet |

It opens the port rather than pinging, because a host can answer ping and still refuse SSH, or block ping and accept SSH. It never logs in.

## Opening a connection

`Enter` on a host runs `ssh <alias>`, so everything in your config applies: keys, jump hosts, ports. How it opens depends on the "Connections" setting:

- **Open in a tab:** the session runs inside lazyssh, next to the host list. You can have several open and switch between them.
- **Take the whole terminal:** lazyssh steps aside and `ssh` gets the terminal to itself. When you log out, lazyssh comes back.
- **Ask every time:** lazyssh asks which of the two.

## The built-in terminal

A tab runs `ssh` on a pseudo-terminal and draws its output with the `vt100` crate. Two things a real terminal does that `vt100` doesn't, lazyssh does itself:

- **It answers the terminal's questions.** Programs ask the terminal what kind it is (`ESC [ c`) and where the cursor is (`ESC [ 6 n`), then wait for the reply. Without an answer, Neovim waited a full second on every start and quit.
- **It passes clipboard copies on.** When a remote program copies with OSC 52, lazyssh hands that to your real terminal, which puts it on your clipboard. See [Copy and paste](copy-and-paste.md).

Your settings and theme are saved in `~/.config/ssh-manager/theme.json`.
