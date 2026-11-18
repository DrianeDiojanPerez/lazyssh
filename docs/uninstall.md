# Uninstall

Linux:

```
sudo rm /usr/local/bin/lazyssh
```

Windows: delete `lazyssh.exe` from `~/bin`, or from wherever you put it.

Your settings stay in `~/.config/ssh-manager/`. Delete that folder to remove them too.

`~/.ssh/config` is yours and stays as it is. So do the `~/.ssh/config.backup_*` copies lazyssh made. Delete those by hand if you don't need them.
