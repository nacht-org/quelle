# Troubleshooting

This guide covers common issues you might encounter while using Quelle and how to solve them.

## Installation Issues

### Rust Not Found

**Problem**: `cargo` or `rustc` commands not found

**Solution**:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add to PATH (add to your shell profile)
export PATH="$HOME/.cargo/bin:$PATH"

# Reload your shell
source ~/.bashrc  # or ~/.zshrc
```

### Build Failures

**Problem**: `cargo build` fails with compilation errors

**Solution**:
```bash
# Update Rust to latest stable
rustup update

# Clean previous builds
cargo clean

# Try building again
cargo build --release -p quelle_cli
```

### Extension Build Fails

**Problem**: `just build-extension dragontea` fails

**Solutions**:
```bash
# Make sure you have WASM target
rustup target add wasm32-unknown-unknown

# Install/update cargo-component
cargo install cargo-component --force

# Make sure just is installed
cargo install just

# Try building again
just build-extension dragontea
```

## Runtime Issues

### "No extension found for URL"

**Problem**: Can't fetch from a URL that should be supported

**Solutions**:
1. Check if you have any stores configured:
   ```bash
   quelle store list
   ```

2. Check if extensions are available:
   ```bash
   quelle list
   ```

3. Build and add the extension manually:
   ```bash
   # Build extension
   just build-extension dragontea
   
   # Create store if needed
   mkdir ./my-extensions
   quelle store add local ./my-extensions --name "dev"
   
   # Copy extension to store
   cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
   ```

### Store Health Issues

**Problem**: `quelle store health` shows errors

**Common causes and fixes**:

- **Directory doesn't exist**:
  ```bash
  mkdir -p ./path/to/store
  ```

- **Permission denied**:
  ```bash
  chmod 755 ./path/to/store
  ```

- **Store not configured**:
  ```bash
  quelle store add local ./path/to/store --name "my-store"
  ```

### Extension Installation Fails

**Problem**: `quelle extension install` doesn't work

**Current limitation**: Extension installation from stores is limited. For now:

1. Build extensions manually:
   ```bash
   just build-extension dragontea
   ```

2. Copy to store directory:
   ```bash
   cp target/wasm32-unknown-unknown/release/extension_dragontea.wasm ./my-extensions/
   ```

## CLI Issues

### Command Not Found

**Problem**: `quelle: command not found`

**Solutions**:
1. Use full path:
   ```bash
   ./target/release/quelle_cli --help
   ```

2. Add to PATH:
   ```bash
   # Copy to system location
   sudo cp target/release/quelle_cli /usr/local/bin/quelle
   
   # Or create alias in your shell profile
   alias quelle="./target/release/quelle_cli"
   ```

### Permission Denied

**Problem**: Can't write to directories or execute files

**Solutions**:
```bash
# Make binary executable
chmod +x target/release/quelle_cli

# Create directories with proper permissions
mkdir -p ~/.local/bin
chmod 755 ~/.local/bin

# For system-wide install (use sudo)
sudo cp target/release/quelle_cli /usr/local/bin/quelle
```

## Data Issues

### Config File Corruption

**Problem**: Strange errors about configuration

**Solution**:
```bash
# Remove config file (will recreate automatically)
rm -rf ./data/config.json

# Re-add your stores
quelle store add local ./my-extensions --name "dev"
```

### Registry Issues

**Problem**: Extensions not showing up correctly

**Solution**:
```bash
# Clear registry data
rm -rf ./data/registry/

# Re-add stores and extensions
quelle store add local ./my-extensions --name "dev"
```

## Website/Network Issues

### Fetch Failures

**Problem**: Can't fetch novels or chapters from websites

**Common causes**:
- **Website is down**: Check if you can access the site in a browser
- **Rate limiting**: The site may be blocking automated requests
- **Changed website structure**: The extension may be outdated
- **Network issues**: Check your internet connection

**Solutions**:
1. Verify URL in browser first
2. Try again later (rate limiting)
3. Check if extension needs updating (future feature)

### Timeout Errors

**Problem**: Requests take too long and timeout

**Temporary workarounds**:
- Try again later when the site is less busy
- Check your internet connection speed
- Some sites may be slow or geographically distant

## Development Issues

### Extension Development

**Problem**: Working on your own extensions

**Common issues**:
- **WIT interface errors**: Make sure you're implementing the correct interfaces
- **Build failures**: Check that dependencies are correct in Cargo.toml
- **Runtime errors**: Add logging to debug issues

**Resources**:
- Look at existing extensions in `extensions/` directory
- Check the WIT interfaces in `wit/` directory

### Missing Dependencies

**Problem**: Build failures due to missing crates

**Solution**:
```bash
# Update dependencies
cargo update

# Or try a clean build
cargo clean
cargo build --release -p quelle_cli
```

## Getting Help

### Debug Information

When asking for help, include:

1. **System information**:
   ```bash
   uname -a
   rustc --version
   cargo --version
   ```

2. **Quelle status**:
   ```bash
   quelle status
   quelle store list
   quelle store health
   ```

3. **Error messages**: Copy the exact error message
4. **Steps to reproduce**: What you were trying to do

### Where to Get Help

- **GitHub Issues**: [https://github.com/nacht-org/quelle/issues](https://github.com/nacht-org/quelle/issues)
- **Discussions**: Check project discussions for community help
- **Documentation**: Review other sections of this book

### Filing Bug Reports

Include:
- Operating system and version
- Rust version (`rustc --version`)
- Exact commands that failed
- Complete error messages
- Steps to reproduce the issue

## Common Error Messages

### "No such file or directory"
- Check that file paths are correct
- Make sure directories exist
- Verify permissions

### "Permission denied"
- Use `chmod` to fix file permissions
- Use `sudo` for system-wide installation
- Check directory ownership

### "Connection refused" or "Network error"
- Check internet connection
- Verify website URLs are correct
- Try again later (may be temporary)

### "WASM component error"
- Extension may be corrupted
- Rebuild extension: `just build-extension <name>`
- Check extension compatibility

Remember: Quelle is in early development, so some issues are expected. Many problems will be resolved as the project matures. When in doubt, try rebuilding everything from scratch!