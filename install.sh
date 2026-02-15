#!/bin/bash

set -e  # Exit on error

# Configuration
REPO_OWNER="DrianeDiojanPerez"
REPO_NAME="lazyssh"
BINARY_NAME="lazyssh"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

detect_os() {
    case "$(uname -s)" in
        Linux*)     OS="linux";;
        Darwin*)    OS="darwin";;
        CYGWIN*|MINGW*|MSYS*) OS="windows";;
        *)
            print_error "Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac
    print_info "Detected OS: $OS"
}

detect_arch() {
    ARCH="$(uname -m)"
    case "$ARCH" in
        x86_64|amd64)   ARCH="x86_64";;
        aarch64|arm64)  ARCH="arm64";;
        armv7l)         ARCH="armv7";;
        i386|i686)      ARCH="386";;
        *)
            print_error "Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac
    print_info "Detected architecture: $ARCH"
}

get_latest_version() {
    print_info "Fetching latest release version..."

    # Try to get latest release using GitHub API
    LATEST_VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO_OWNER}/${REPO_NAME}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$LATEST_VERSION" ]; then
        print_error "Failed to fetch latest version from GitHub"
        exit 1
    fi

    print_info "Latest version: $LATEST_VERSION"
}

# Check if already installed and compare versions
check_existing_version() {
    # Determine where binary would be installed
    if [ "$OS" = "windows" ]; then
        INSTALL_PATH="$HOME/bin/${BINARY_NAME}.exe"
    else
        INSTALL_PATH="/usr/local/bin/$BINARY_NAME"
    fi

    if [ -f "$INSTALL_PATH" ]; then
        print_info "Existing installation found at $INSTALL_PATH"

        # Try to get current version
        if CURRENT_VERSION=$("$INSTALL_PATH" --version 2>/dev/null | head -n 1); then
            print_info "Current version: $CURRENT_VERSION"

            # Simple comparison - if versions match
            if echo "$CURRENT_VERSION" | grep -q "$LATEST_VERSION"; then
                print_info "✓ Already up to date!"
                echo ""
                read -p "Reinstall anyway? (y/N) " -n 1 -r
                echo ""
                if [[ ! $REPLY =~ ^[Yy]$ ]]; then
                    print_info "Installation cancelled"
                    exit 0
                fi
            else
                print_info "Update available: $CURRENT_VERSION → $LATEST_VERSION"
            fi
        fi
    fi
}

# Construct download URL
construct_download_url() {
    # Capitalize first letter of OS for the release filename
    OS_CAPITALIZED="$(echo "$OS" | sed 's/^./\U&/')"

    # Determine file extension
    if [ "$OS" = "windows" ]; then
        EXT="zip"
    else
        EXT="tar.gz"
    fi

    # lazyssh uses pattern: lazyssh_{OS}_{arch}.{ext}
    FILENAME="${BINARY_NAME}_${OS_CAPITALIZED}_${ARCH}.${EXT}"

    DOWNLOAD_URL="https://github.com/${REPO_OWNER}/${REPO_NAME}/releases/download/${LATEST_VERSION}/${FILENAME}"
    print_info "Download URL: $DOWNLOAD_URL"
}

# Download and extract binary
download_binary() {
    print_info "Downloading ${FILENAME}..."

    TEMP_FILE="/tmp/${FILENAME}"

    if ! curl -fsSL "$DOWNLOAD_URL" -o "$TEMP_FILE"; then
        print_error "Failed to download binary from $DOWNLOAD_URL"
        print_warn "Please check if the release exists and the naming pattern is correct"
        exit 1
    fi

    print_info "Download completed"

    # Extract archive
    print_info "Extracting archive..."
    TEMP_DIR="/tmp/lazyssh-install-$$"
    mkdir -p "$TEMP_DIR"

    # Extract based on file extension
    if [[ "$FILENAME" == *.tar.gz ]]; then
        if ! tar -xzf "$TEMP_FILE" -C "$TEMP_DIR"; then
            print_error "Failed to extract tar.gz archive"
            rm -rf "$TEMP_DIR" "$TEMP_FILE"
            exit 1
        fi
    elif [[ "$FILENAME" == *.zip ]]; then
        if ! unzip -q "$TEMP_FILE" -d "$TEMP_DIR"; then
            print_error "Failed to extract zip archive"
            print_warn "Make sure 'unzip' is installed on your system"
            rm -rf "$TEMP_DIR" "$TEMP_FILE"
            exit 1
        fi
    else
        print_error "Unknown archive format: $FILENAME"
        rm -rf "$TEMP_DIR" "$TEMP_FILE"
        exit 1
    fi

    # Find the binary (should be in the extracted directory)
    EXTRACTED_BINARY=$(find "$TEMP_DIR" -type f -name "$BINARY_NAME" -o -name "${BINARY_NAME}.exe" | head -n 1)

    if [ -z "$EXTRACTED_BINARY" ]; then
        print_error "Could not find binary in extracted archive"
        rm -rf "$TEMP_DIR" "$TEMP_FILE"
        exit 1
    fi

    # Move to a known location for installation
    TEMP_BINARY="/tmp/${BINARY_NAME}-binary-$$"
    mv "$EXTRACTED_BINARY" "$TEMP_BINARY"

    # Cleanup
    rm -rf "$TEMP_DIR" "$TEMP_FILE"

    print_info "Extraction completed"
}

# Install binary
install_binary() {
    print_info "Installing ${BINARY_NAME}..."

    # Determine installation directory
    if [ "$OS" = "windows" ]; then
        # For Windows (Git Bash/WSL), install to a location in PATH
        INSTALL_DIR="$HOME/bin"
        INSTALL_PATH="$INSTALL_DIR/${BINARY_NAME}.exe"

        # Create directory if it doesn't exist
        mkdir -p "$INSTALL_DIR"

        mv "$TEMP_BINARY" "$INSTALL_PATH"
        chmod +x "$INSTALL_PATH"
    else
        # For Linux/macOS, install to /usr/local/bin (requires sudo)
        INSTALL_DIR="/usr/local/bin"
        INSTALL_PATH="$INSTALL_DIR/$BINARY_NAME"

        # Check if we have write permission
        if [ -w "$INSTALL_DIR" ]; then
            # We can write directly
            mv "$TEMP_BINARY" "$INSTALL_PATH"
            chmod +x "$INSTALL_PATH"
        else
            # Need sudo
            print_info "Installing to $INSTALL_DIR requires sudo privileges"
            if ! sudo mv "$TEMP_BINARY" "$INSTALL_PATH"; then
                print_error "Failed to install binary. Please run with sudo or install manually."
                exit 1
            fi
            sudo chmod +x "$INSTALL_PATH"
        fi
    fi

    print_info "Installed to: $INSTALL_PATH"
}

# Check if directory is in PATH (only needed for Windows)
check_path() {
    if [ "$OS" = "windows" ]; then
        if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
            print_warn "$INSTALL_DIR is not in your PATH"
            print_warn "Add the following line to your shell configuration file (~/.bashrc, ~/.zshrc, etc.):"
            echo ""
            echo "    export PATH=\"\$HOME/bin:\$PATH\""
            echo ""
            print_warn "Then restart your terminal or run: source ~/.bashrc"
        else
            print_info "$INSTALL_DIR is already in your PATH"
        fi
    fi
}

# Main installation flow
main() {
    echo ""
    print_info "Starting installation of ${BINARY_NAME}..."
    echo ""

    detect_os
    detect_arch
    get_latest_version
    check_existing_version
    construct_download_url
    download_binary
    install_binary
    check_path

    echo ""
    print_info "✓ Installation complete!"
    print_info "Run '${BINARY_NAME} --help' to get started"
    echo ""
}

# Run main function
main
