# Troubleshooting

This section covers common issues you might encounter while using Quelle and how to solve them.

## Installation Issues

### Rust/Cargo Not Found

**Problem:** `cargo: command not found` or similar errors.

**Solution:**
1. Install Rust: https://rustup.rs/
2. Restart your terminal
3. Verify installation: `cargo --version`

### Build Failures

**Problem:** Compilation errors when building Quelle.

**Solutions:**
1. Make sure you have the latest Rust stable:
   ```bash
   rustup update stable
   ```

2. Install required targets:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

3. Install cargo-component:
   ```bash
   cargo install cargo-component
   ```

4. Clean and rebuild:
   ```bash
   cargo clean
   cargo build --release
   ```

## Extension Issues

### "No extension found for URL"

**Problem:** Quelle can't find an extension for a website URL.

**Causes and Solutions:**

1. **Extension not installed:**
   ```bash
   quelle extensions list
   quelle extensions install dragontea
   ```

2. **Wrong URL format:**
   - Check if the URL matches the expected pattern
   - Try the novel's main page URL instead of a chapter URL

3. **Extension not built:**
   ```bash
   just build-extension dragontea
   # Copy to your store directory if needed
   ```

### Extension Crashes or Errors

**Problem:** Extension fails with runtime errors.

**Solutions:**

1. **Check the URL is accessible:**
   - Open the URL in your browser
   - Make sure the site is working

2. **Update extensions:**
   ```bash
   quelle extensions update all
   ```

3. **Check for site changes:**
   - Websites sometimes change their structure
   - Extensions may need updates

4. **Use verbose mode for debugging:**
   ```bash
   quelle --verbose fetch novel https://example.com/novel
   ```

## Network Issues

### Timeouts and Connection Errors

**Problem:** Network requests fail or timeout.

**Solutions:**

1. **Check internet connection:**
   - Test with other websites
   - Try accessing the site in a browser

2. **Retry the operation:**
   - Network issues are often temporary
   - Quelle will resume where it left off

3. **Check if site is blocking requests:**
   - Some sites may rate-limit or block automated requests
   - Try waiting and retrying later

### Download Failures

**Problem:** Novel or chapter downloads fail partway through.

**Solutions:**

1. **Resume the download:**
   ```bash
   quelle update "Novel Title"
   ```

2. **Check available disk space:**
   ```bash
   df -h  # On Linux/macOS
   # Check disk space on Windows
   ```

3. **Verify network stability:**
   - Large downloads need stable connections
   - Consider downloading in smaller batches

## Library Issues

### Novels Not Appearing

**Problem:** Added novels don't show up in library list.

**Solutions:**

1. **Check library location:**
   ```bash
   quelle config get data_dir
   ls -la /path/to/data/dir
   ```

2. **Check for errors during add:**
   ```bash
   quelle --verbose add https://example.com/novel
   ```

3. **Verify library integrity:**
   ```bash
   quelle library cleanup
   quelle status
   ```

### Reading Issues

**Problem:** Can't read chapters or content appears corrupted.

**Solutions:**

1. **List available chapters:**
   ```bash
   quelle library chapters "Novel Title"
   ```

2. **Re-download the chapter:**
   ```bash
   quelle update "Novel Title"
   ```

3. **Check file permissions:**
   ```bash
   ls -la ~/.local/share/quelle/  # Default location
   ```

## Store Issues

### Store Not Accessible

**Problem:** Can't access configured stores.

**Solutions:**

1. **List configured stores:**
   ```bash
   quelle store list
   ```

2. **Check store health:**
   ```bash
   quelle store info store-name
   ```

3. **Update store data:**
   ```bash
   quelle store update store-name
   ```

4. **For Git stores, check authentication:**
   ```bash
   # Re-add with correct credentials
   quelle store remove old-store --force
   quelle store add git new-store https://repo.git --token YOUR_TOKEN
   ```

### Permission Errors

**Problem:** Permission denied errors when accessing stores.

**Solutions:**

1. **Check directory permissions:**
   ```bash
   ls -la /path/to/store/directory
   chmod 755 /path/to/store/directory  # If needed
   ```

2. **For Git stores, check repository permissions:**
   - Make sure you have read access to the repository
   - Check if authentication tokens are valid

## Configuration Issues

### Config File Problems

**Problem:** Configuration errors or corrupted config.

**Solutions:**

1. **View current configuration:**
   ```bash
   quelle config show
   ```

2. **Reset to defaults:**
   ```bash
   quelle config reset --force
   ```

3. **Fix specific settings:**
   ```bash
   quelle config set data_dir /correct/path
   ```

### Storage Location Issues

**Problem:** Can't write to storage directory.

**Solutions:**

1. **Check permissions:**
   ```bash
   ls -la ~/.local/share/quelle/
   mkdir -p ~/.local/share/quelle/  # Create if needed
   ```

2. **Use custom location:**
   ```bash
   quelle --storage-path /custom/path library list
   # Or set permanently:
   quelle config set data_dir /custom/path
   ```

## Performance Issues

### Slow Operations

**Problem:** Commands take a long time to complete.

**Solutions:**

1. **Use progress indicators:**
   ```bash
   quelle --verbose add https://example.com/novel
   ```

2. **Limit concurrent operations:**
   - Add novels one at a time for large libraries
   - Use `--max-chapters` for testing

3. **Check available resources:**
   - Ensure adequate disk space
   - Monitor memory usage

### High Memory Usage

**Problem:** Quelle uses too much memory.

**Solutions:**

1. **Process novels individually:**
   ```bash
   quelle add https://example.com/novel --max-chapters 50
   ```

2. **Clean up regularly:**
   ```bash
   quelle library cleanup
   ```

## Getting Help

### Debug Information

When reporting issues, include:

1. **Version information:**
   ```bash
   quelle --version
   ```

2. **System status:**
   ```bash
   quelle status
   ```

3. **Verbose output:**
   ```bash
   quelle --verbose --dry-run add https://problem-url.com/novel
   ```

4. **Configuration:**
   ```bash
   quelle config show
   quelle store list
   quelle extensions list
   ```

### Log Files

Check log files for detailed error information:
- **Linux/macOS:** `~/.local/share/quelle/logs/`
- **Windows:** `%APPDATA%\quelle\logs\`

### Common Error Messages

**"Extension validation failed"**
- Extension file is corrupted or incompatible
- Rebuild the extension or download a fresh copy

**"Store sync failed"**
- Network connectivity issues
- Authentication problems for Git stores
- Try updating the store manually

**"Novel parsing failed"**
- Website structure has changed
- Extension needs updating
- Try a different URL format

**"Chapter download incomplete"**
- Network interrupted during download
- Resume with `quelle update "Novel Title"`

### Still Need Help?

If these solutions don't work:

1. **Check the project repository:** https://github.com/nacht-org/quelle
2. **Create an issue** with detailed information about your problem
3. **Include system information, error messages, and steps to reproduce**

Remember: Quelle is in active development, so some issues may be due to ongoing changes or incomplete features.