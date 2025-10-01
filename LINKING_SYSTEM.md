# Linking System Implementation

This document describes the linking system implemented for the Quelle local store, which creates navigational relationships between store manifests, extension manifests, and their associated files.

## Overview

The linking system establishes a clear hierarchy of references that enables:
- **Discovery**: Finding all files associated with an extension
- **Integrity**: Verifying files haven't been corrupted using checksums
- **Navigation**: Moving between related manifests and files
- **Caching**: Determining if files need updates based on checksums
- **Metadata**: Associating additional information with files

## Architecture

The linking system follows a two-level hierarchy:

```
store.json (Store Manifest)
├─ extensions[0].manifest_path → extensions/ext.id/version/manifest.json
│  ├─ wasm_file.path → ./extension.wasm
│  └─ assets[0].path → ./assets/icon.png
└─ extensions[1].manifest_path → extensions/other.ext/version/manifest.json
   ├─ wasm_file.path → ./extension.wasm
   └─ assets[0].path → ./README.md
```

## Data Structures

### Store Manifest Links

The store manifest (`store.json`) contains links to extension manifests:

```json
{
  "extensions": [
    {
      "id": "example.extension",
      "name": "Example Extension",
      "version": "1.0.0",
      // ... other fields
      "manifest_path": "extensions/example.extension/1.0.0/manifest.json",
      "manifest_checksum": "blake3:abc123..."
    }
  ]
}
```

**Fields:**
- `manifest_path`: Relative path from store root to extension manifest
- `manifest_checksum`: Blake3 checksum of the manifest file for integrity verification

### Extension Manifest Links

Extension manifests (`manifest.json`) contain links to their associated files:

```json
{
  "id": "example.extension",
  "name": "Example Extension",
  // ... existing fields
  "wasm_file": {
    "path": "./extension.wasm",
    "checksum": "blake3:def456...",
    "size": 524288
  },
  "assets": [
    {
      "name": "icon.png",
      "path": "./assets/icon.png",
      "checksum": "blake3:ghi789...",
      "size": 2048,
      "type": "icon"
    },
    {
      "name": "README.md", 
      "path": "./README.md",
      "checksum": "blake3:jkl012...",
      "size": 1024,
      "type": "documentation"
    }
  ]
}
```

**FileReference Structure:**
- `path`: Relative path from manifest location
- `checksum`: Blake3 checksum for integrity verification  
- `size`: File size in bytes

**AssetReference Structure:**
- `name`: Asset identifier/filename
- `path`: Relative path from manifest location
- `checksum`: Blake3 checksum for integrity verification
- `size`: File size in bytes
- `type`: Asset type ("icon", "documentation", "asset", etc.)

## Implementation Details

### Rust Data Structures

```rust
// Store manifest extension summary
pub struct ExtensionSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    // ... existing fields
    pub manifest_path: Option<String>,
    pub manifest_checksum: Option<String>,
}

// File reference with integrity information
pub struct FileReference {
    pub path: String,
    pub checksum: String,
    pub size: u64,
}

// Asset reference with additional metadata
pub struct AssetReference {
    pub name: String,
    pub path: String,
    pub checksum: String,
    pub size: u64,
    pub asset_type: String,
}

// Extension manifest with file links
pub struct ExtensionManifest {
    // ... existing fields
    pub wasm_file: Option<FileReference>,
    pub assets: Vec<AssetReference>,
}
```

### Link Generation

Links are automatically generated during extension publishing:

1. **Store Manifest Links**: Generated when `save_store_manifest()` is called
   - Scans extension directories
   - Calculates manifest file checksums
   - Creates relative paths from store root

2. **Extension Manifest Links**: Generated during `publish()` operation
   - Creates `FileReference` for WASM component
   - Creates `AssetReference` for each asset file
   - All paths are relative to manifest location

### Checksum Calculation

All checksums use Blake3 algorithm for security and performance:

```rust
let checksum = format!("blake3:{}", blake3::hash(data).to_hex());
```

### Backward Compatibility

The linking system is designed for backward compatibility:
- All new fields are `Option<T>` types
- Existing manifests without links continue to work
- Links are populated during normal operations (publish, refresh)

## Usage Examples

### Verifying File Integrity

```rust
// Verify WASM file matches its reference
if let Some(wasm_file) = &manifest.wasm_file {
    let wasm_data = fs::read(&wasm_file.path).await?;
    if wasm_file.verify(&wasm_data) {
        println!("✅ WASM file integrity verified");
    } else {
        println!("❌ WASM file corrupted");
    }
}
```

### Discovering Extension Assets

```rust
// List all assets for an extension
for asset in &manifest.assets {
    println!("Asset: {} ({} bytes, type: {})", 
             asset.name, asset.size, asset.asset_type);
}
```

### Following Links from Store to Files

```rust
// Navigate from store manifest to extension files
let store_manifest = store.get_local_store_manifest().await?;
for extension in &store_manifest.extensions {
    if let Some(manifest_path) = &extension.manifest_path {
        let full_path = store_root.join(manifest_path);
        let ext_manifest = load_extension_manifest(&full_path).await?;
        
        // Now access extension files
        if let Some(wasm_file) = &ext_manifest.wasm_file {
            let wasm_path = full_path.parent().unwrap().join(&wasm_file.path);
            // Process WASM file...
        }
    }
}
```

## Benefits

### Security & Integrity
- **Tamper Detection**: Checksums detect file modifications
- **Completeness Verification**: Ensure all expected files are present
- **Version Consistency**: Link checksums help detect version mismatches

### Performance
- **Selective Updates**: Only update files with changed checksums
- **Bandwidth Optimization**: File sizes enable transfer planning
- **Cache Efficiency**: Checksums provide cache keys

### Maintainability
- **Self-Documenting**: Store structure is explicit in manifests
- **Automated Verification**: Built-in integrity checking
- **Orphan Detection**: Missing files are easily identified

### Developer Experience
- **Clear Navigation**: Follow links between related files
- **Asset Discovery**: Automatically find all extension resources
- **Debugging Support**: Link information aids troubleshooting

## Migration

### For Existing Stores

1. **Automatic Migration**: Links are added during normal operations
2. **Manual Regeneration**: Use the regeneration utility for immediate updates
3. **Gradual Enhancement**: Extension manifests get links when republished

### For Store Implementations

1. **Optional Fields**: New linking fields are optional for compatibility
2. **Local Store Only**: Linking logic contained within `LocalStore`
3. **No Trait Changes**: Generic store traits remain unchanged

## Testing

The linking system includes comprehensive tests:

- **Unit Tests**: Verify link generation and verification
- **Integration Tests**: Test full publish → link → verify cycle  
- **Compatibility Tests**: Ensure backward compatibility
- **Performance Tests**: Validate checksum calculation efficiency

## Future Enhancements

Potential future improvements:

1. **Cross-Store Links**: Links between federated stores
2. **Differential Updates**: Delta syncing based on checksums
3. **Compression Support**: Links to compressed asset bundles
4. **Signature Verification**: Cryptographic signatures for assets
5. **Metadata Expansion**: Rich metadata for specialized asset types