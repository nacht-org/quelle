# Storage Fix: mark_stored Method Now Properly Used

## Problem

The `mark_stored` method existed in `ChapterInfo` but was never called by the storage system, leading to inconsistent state between in-memory objects and stored data.

## Before Fix

```rust
// Storage system didn't update in-memory objects
let mut chapter_info = ChapterInfo::new(1, "https://example.com/ch1".to_string(), "Chapter 1".to_string(), 1);
assert!(!chapter_info.has_content()); // false

// Store content
storage.store_chapter_content(&novel_id, 1, "https://example.com/ch1", &content).await?; // Returned ()

// Chapter info object was NOT updated
assert!(!chapter_info.has_content()); // Still false! ❌
assert!(chapter_info.content_size().is_none()); // Still None! ❌

// Only fresh objects from storage showed correct status
let fresh_chapters = storage.list_chapters(&novel_id).await?;
assert!(fresh_chapters[0].has_content()); // true ✅
```

## After Fix

```rust
// Storage system now returns updated ChapterInfo objects
let mut chapter_info = ChapterInfo::new(1, "https://example.com/ch1".to_string(), "Chapter 1".to_string(), 1);
assert!(!chapter_info.has_content()); // false

// Store content - now returns updated ChapterInfo
let updated_chapter = storage.store_chapter_content(&novel_id, 1, "https://example.com/ch1", &content).await?;

// Returned object has correct status (mark_stored was called internally)
assert!(updated_chapter.has_content()); // true ✅
assert_eq!(updated_chapter.content_size().unwrap(), content.data.len() as u64); // correct size ✅

// Original object can be manually updated if needed
chapter_info.mark_stored(content.data.len() as u64);
assert!(chapter_info.has_content()); // Now true ✅
```

## Changes Made

### 1. Updated Storage Trait

```rust
// Before
async fn store_chapter_content(&self, novel_id: &NovelId, volume_index: i32, chapter_url: &str, content: &ChapterContent) -> Result<()>;
async fn delete_chapter_content(&self, novel_id: &NovelId, volume_index: i32, chapter_url: &str) -> Result<bool>;

// After  
async fn store_chapter_content(&self, novel_id: &NovelId, volume_index: i32, chapter_url: &str, content: &ChapterContent) -> Result<ChapterInfo>;
async fn delete_chapter_content(&self, novel_id: &NovelId, volume_index: i32, chapter_url: &str) -> Result<Option<ChapterInfo>>;
```

### 2. Updated Implementation

The filesystem storage now:
- Creates `ChapterInfo` objects for stored/deleted chapters
- Calls `mark_stored()` or `mark_removed()` on these objects
- Returns the updated objects to callers

### 3. Updated CLI Commands

All CLI commands that store chapter content now receive and can use the updated `ChapterInfo` objects:

```rust
// Before
storage.store_chapter_content(&novel_id, volume_index, &chapter_url, &content).await?;

// After
let updated_chapter = storage.store_chapter_content(&novel_id, volume_index, &chapter_url, &content).await?;
// Can now use updated_chapter.has_content(), updated_chapter.content_size(), etc.
```

## Benefits

1. **Architectural Consistency**: `mark_stored` is now properly used by the storage system
2. **Immediate State Updates**: Callers get updated objects without needing to re-query storage
3. **Better Performance**: No need to call `list_chapters` again just to get updated status
4. **Cleaner API**: Return values provide useful information about what was stored/deleted

## Example Usage

```rust
use quelle_storage::{FilesystemStorage, BookStorage, ChapterContent};

// Initialize storage
let storage = FilesystemStorage::new("/path/to/storage");
let novel_id = storage.store_novel(&novel).await?;

// Store chapter content - get updated info back
let content = ChapterContent { data: "Chapter content...".to_string() };
let updated_chapter = storage.store_chapter_content(
    &novel_id, 
    1, 
    "https://example.com/chapter-1", 
    &content
).await?;

// Use the updated chapter info immediately
println!("Stored chapter: {} ({} bytes)", 
    updated_chapter.chapter_title, 
    updated_chapter.content_size().unwrap()
);

// Delete content - get updated info back
let deleted_chapter = storage.delete_chapter_content(&novel_id, 1, "https://example.com/chapter-1").await?;
if let Some(chapter) = deleted_chapter {
    println!("Deleted chapter: {}", chapter.chapter_title);
    assert!(!chapter.has_content()); // Correctly shows as not stored
}
```

This fix resolves the architectural inconsistency and makes the storage system more robust and user-friendly.