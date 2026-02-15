dry-release:
    goreleaser release --snapshot --clean --verbose
release:
    goreleaser --clean
release version:
    #!/usr/bin/env bash
    set -euo pipefail

    # Strip 'v' prefix if present
    VERSION="{{version}}"
    VERSION="${VERSION#v}"

    # Validate semver format
    if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
        echo "Error: version must be semver (e.g. 1.0.0)"
        exit 1
    fi

    # Get current version
    CURRENT=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
    echo "$CURRENT -> $VERSION"

    # Update Cargo.toml
    sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

    # Commit and tag
    git add Cargo.toml
    git commit -m "release v$VERSION"
    git tag "v$VERSION"

    echo ""
    echo "Tagged v$VERSION"
    echo "Run 'git push && git push --tags' to publish"
