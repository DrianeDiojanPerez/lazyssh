# lazyssh

A simple SSH connection manager for lazy people.

## Installation

### Quick Install (Recommended)

**Linux / macOS / WSL:**
```bash
curl -fsSL https://raw.githubusercontent.com/DrianeDiojanPerez/lazyssh/refs/heads/master/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/DrianeDiojanPerez/lazyssh/refs/heads/master/install.ps1 | iex
```

### Manual Installation

Download the appropriate binary for your system from the [releases page](https://github.com/DrianeDiojanPerez/lazyssh/releases):

**Linux:**
```bash
# Download for your architecture
wget https://github.com/DrianeDiojanPerez/lazyssh/releases/latest/download/lazyssh_Linux_x86_64.tar.gz

# Extract
tar -xzf lazyssh_Linux_x86_64.tar.gz

# Install globally
sudo mv lazyssh /usr/local/bin/
sudo chmod +x /usr/local/bin/lazyssh
```

**Windows:**
```powershell
# Download the zip file from releases page
# Extract it
# Move lazyssh.exe to a folder in your PATH
```

## Usage

After installation, simply run:
```bash
lazyssh
```

Or check available options:
```bash
lazyssh --help
```

## Updating

To update to the latest version, just run the installation command again:

```bash
# Linux/macOS
curl -fsSL https://raw.githubusercontent.com/DrianeDiojanPerez/lazyssh/refs/heads/master/install.sh | bash

# Windows
irm https://raw.githubusercontent.com/DrianeDiojanPerez/lazyssh/refs/heads/master/install.ps1 | iex
```

The installer will detect if you're already up to date.

## Supported Platforms

- **Linux**: x86_64, ARM64
- **Windows**: x86_64

## Uninstalling

**Linux/macOS:**
```bash
sudo rm /usr/local/bin/lazyssh
```

**Windows:**
```powershell
Remove-Item "$env:USERPROFILE\bin\lazyssh.exe"
```

## Building from Source

```bash
# Clone the repository
git clone https://github.com/DrianeDiojanPerez/lazyssh.git
cd lazyssh

# Build (requires Go)
go build -o lazyssh

# Install
sudo mv lazyssh /usr/local/bin/
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

If you encounter any issues:
1. Check the [issues page](https://github.com/DrianeDiojanPerez/lazyssh/issues)
2. Create a new issue with details about your system and the problem
