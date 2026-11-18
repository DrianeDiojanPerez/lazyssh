# Troubleshooting

- **A host shows a red dot but you can SSH to it:** lazyssh only checks the HostName and Port in the config. A host behind a jump host (`ProxyJump`) can't be reached directly, so it shows red even though `ssh` gets there. Connecting still works.
- **Neovim takes a second to open or quit in a tab:** update to v0.0.8 or newer with `lazyssh --update`. Older versions didn't answer the terminal's questions, and Neovim waited for its timeout.
- **`"+y` in Neovim doesn't reach your clipboard:** check `lazyssh --version` is v0.0.8 or newer, and that your terminal lets programs set the clipboard. If the server has a clipboard tool like `xclip` or `wl-copy`, Neovim uses that instead of OSC 52. Set `vim.g.clipboard = "osc52"` in the server's Neovim config to force it.
- **Pasting into Neovim indents every line:** update to v0.0.8 or newer, which marks pastes as pastes.
- **Selecting text with the mouse doesn't work:** hold `Shift` while you drag.
- **A `Host *` block or comments vanished from `~/.ssh/config`:** a save in lazyssh doesn't keep them below the first `Host` line. See [what a save doesn't keep](how-it-works.md#what-a-save-doesnt-keep), and restore them from the backup.
- **An edit to `~/.ssh/config` went wrong:** every save leaves a copy in `~/.ssh/config.backup_<date>_<time>`. Copy the newest one back over `~/.ssh/config` and press `r`.
- **`lazyssh --update` says the installer couldn't be downloaded:** it needs `curl` and `bash`. Run the install line from the [README](../README.md#install) by hand to see the full error.
- **Hosts you added in another editor don't show up:** press `r` to read the file again.
