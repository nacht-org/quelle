# Max Chapters Feature

This document explains the `--max-chapters` feature in the Quelle CLI, which allows users to limit the number of chapters downloaded when adding novels to their library.

## Overview

The `--max-chapters` option is useful for:
- **Testing**: Quickly test a novel without downloading all chapters
- **Sampling**: Preview a few chapters before committing to the full novel
- **Bandwidth Management**: Limit downloads on slow connections
- **Storage Management**: Control disk space usage
- **Development**: Test extensions with a limited dataset

## Usage

### Basic Syntax

```bash
quelle add <URL> --max-chapters <NUMBER>
```

### Examples

```bash
# Download only the first 5 chapters
quelle add https://example.com/novel --max-chapters 5

# Download only the first chapter for testing
quelle add https://royalroad.com/fiction/12345 --max-chapters 1

# Download first 10 chapters for preview
quelle add https://webnovel.com/book/novel-title --max-chapters 10
```

### Combined with Other Options

```bash
# Preview mode: metadata only (no chapters)
quelle add https://example.com/novel --no-chapters

# Limited download with dry-run
quelle add https://example.com/novel --max-chapters 3 --dry-run

# Test with verbose output
quelle add https://example.com/novel --max-chapters 2 --verbose
```

## Behavior

### Chapter Selection
- Chapters are processed in the order they appear in the novel
- The limit applies to **new downloads only**
- Already downloaded chapters are skipped and don't count toward the limit
- If fewer chapters exist than the limit, all available chapters are downloaded

### Output Examples

#### With Limit Applied
```
📚 Adding novel from: https://example.com/novel
📚 Fetching novel metadata...
✅ Novel metadata fetched
📄 Fetching chapters...
📚 Fetching chapters for novel: https://example.com/novel
📏 Limited to 5 chapters (out of 157 available)
📄 Processing 5 chapters
📥 Fetching: Chapter 1: The Beginning
  ✅ Chapter 1: The Beginning
📥 Fetching: Chapter 2: First Steps
  ✅ Chapter 2: First Steps
...
📊 Fetch complete:
  ✅ Successfully fetched: 5
  ⏭️ Already downloaded: 0
✅ Novel added successfully!
```

#### Without Limit
```
📚 Adding novel from: https://example.com/novel
📚 Fetching novel metadata...
✅ Novel metadata fetched
📄 Fetching chapters...
📚 Fetching chapters for novel: https://example.com/novel
📄 Processing 157 chapters
📥 Fetching: Chapter 1: The Beginning
  ✅ Chapter 1: The Beginning
...
📊 Fetch complete:
  ✅ Successfully fetched: 157
  ⏭️ Already downloaded: 0
✅ Novel added successfully!
```

#### With Already Downloaded Chapters
```
📚 Adding novel from: https://example.com/novel
📚 Fetching chapters for novel: https://example.com/novel
📏 Limited to 5 chapters (out of 157 available)
📄 Processing 5 chapters
  ⏭️ Chapter 1: The Beginning (already downloaded)
  ⏭️ Chapter 2: First Steps (already downloaded)
📥 Fetching: Chapter 3: New Territory
  ✅ Chapter 3: New Territory
📥 Fetching: Chapter 4: Challenges
  ✅ Chapter 4: Challenges
📥 Fetching: Chapter 5: Progress
  ✅ Chapter 5: Progress
📊 Fetch complete:
  ✅ Successfully fetched: 3
  ⏭️ Already downloaded: 2
```

## Dry Run Mode

Test the feature without actually downloading:

```bash
# See what would be downloaded
quelle add https://example.com/novel --max-chapters 5 --dry-run
# Output: Would add novel from: https://example.com/novel
#         Would fetch first 5 chapters

# Compare with unlimited
quelle add https://example.com/novel --dry-run  
# Output: Would add novel from: https://example.com/novel
#         Would fetch all chapters

# Metadata only
quelle add https://example.com/novel --no-chapters --dry-run
# Output: Would add novel from: https://example.com/novel
```

## Use Cases

### 1. Extension Testing
```bash
# Test new extension with minimal download
quelle add https://new-site.com/novel --max-chapters 1
```

### 2. Novel Sampling
```bash
# Download first few chapters to decide if you like the novel
quelle add https://example.com/long-novel --max-chapters 3
```

### 3. Development and Debugging
```bash
# Quick test during development
quelle add https://test-site.com/novel --max-chapters 2 --verbose
```

### 4. Bandwidth Conservation
```bash
# On slow connection, download just a few chapters at a time
quelle add https://example.com/novel --max-chapters 10
# Later, use update to get more chapters
quelle update "Novel Title"
```

### 5. Storage Management
```bash
# Limit initial download for space-constrained environments
quelle add https://example.com/huge-novel --max-chapters 5
```

## Integration with Update Command

After adding a novel with `--max-chapters`, you can download more chapters using the update command:

```bash
# Initially download first 5 chapters
quelle add https://example.com/novel --max-chapters 5

# Later, download remaining chapters
quelle update "Novel Title"

# Or update all novels
quelle update all
```

The update command will fetch any missing chapters beyond the initial limit.

## Error Handling

### Invalid Limits
```bash
# Zero chapters (valid - only downloads metadata)
quelle add https://example.com/novel --max-chapters 0
# Equivalent to --no-chapters

# Negative numbers are rejected by the CLI parser
quelle add https://example.com/novel --max-chapters -1
# Error: invalid value '-1' for '--max-chapters <MAX_CHAPTERS>': invalid digit found in string
```

### Novel Not Found
If the novel URL is invalid or the extension fails, the error occurs before chapter limiting:

```bash
quelle add https://invalid-url.com/novel --max-chapters 5
# ❌ Novel not found with URL: https://invalid-url.com/novel
```

### Extension Errors
Chapter limiting doesn't affect extension-level errors:

```bash
# If extension fails on chapter 3 out of 5 requested
📏 Limited to 5 chapters (out of 20 available)
📄 Processing 5 chapters
  ✅ Chapter 1: Success
  ✅ Chapter 2: Success  
  ❌ Failed to fetch Chapter 3: Network error
  ✅ Chapter 4: Success
  ✅ Chapter 5: Success
📊 Fetch complete:
  ✅ Successfully fetched: 4
  ❌ Failed: 1
```

## Best Practices

### For Testing
- Use `--max-chapters 1` for quick extension validation
- Combine with `--dry-run` to preview behavior
- Use `--verbose` for detailed logging during testing

### For Production Use
- Start with small limits (3-5 chapters) for new novels
- Use the update command to fetch remaining chapters
- Consider storage and bandwidth constraints

### For Development
- Test with various chapter limits to ensure robustness
- Verify behavior with edge cases (0 chapters, more than available)
- Test with novels that have already downloaded chapters

## Implementation Notes

The feature works by:
1. Fetching the complete chapter list from storage
2. Truncating the list to the specified limit
3. Processing only the limited chapters
4. Skipping already downloaded chapters within that limit

This approach ensures:
- Consistent behavior across different novel sources
- Proper handling of already downloaded content
- Accurate progress reporting
- Integration with existing update workflows

## Troubleshooting

### Chapters Not Limited
- Ensure you're using `--max-chapters` not `--max_chapters`
- Check that the number is positive
- Verify the novel has more chapters than your limit

### Unexpected Chapter Count
- Remember that already downloaded chapters are skipped
- The limit applies to the initial chapter list, not downloaded count
- Use `quelle library chapters "Novel Title"` to see current status

### Performance Issues
- Very large limits may still be slow to process
- Consider using smaller limits for initial testing
- Network conditions affect download speed regardless of limit