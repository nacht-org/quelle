use eyre::Result;
use quelle_storage::{
    backends::filesystem::FilesystemStorage,
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};

use crate::cli::ExportCommands;

pub async fn handle_export_command(
    cmd: ExportCommands,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    match cmd {
        ExportCommands::Epub {
            novel_id,
            chapters,
            output,
            template,
            combine_volumes,
            updated,
        } => {
            handle_export_epub(
                novel_id,
                chapters,
                output,
                template,
                combine_volumes,
                updated,
                storage,
                dry_run,
            )
            .await
        }
        ExportCommands::Pdf { novel_id, output } => {
            handle_export_pdf(novel_id, output, dry_run).await
        }
        ExportCommands::Html { novel_id, output } => {
            handle_export_html(novel_id, output, dry_run).await
        }
        ExportCommands::Txt { novel_id, output } => {
            handle_export_txt(novel_id, output, dry_run).await
        }
    }
}

async fn handle_export_epub(
    novel_id: String,
    _chapters: Option<String>,
    _output: Option<String>,
    _template: Option<String>,
    _combine_volumes: bool,
    _updated: bool,
    storage: &FilesystemStorage,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would export to EPUB: {}", novel_id);
        return Ok(());
    }

    // Check if novel exists
    if novel_id != "all" {
        let id = NovelId::new(novel_id.clone());
        match storage.get_novel(&id).await? {
            Some(novel) => {
                println!("📚 Exporting novel: {}", novel.title);
                println!("  Authors: {}", novel.authors.join(", "));

                // List chapters to export
                let chapters = storage.list_chapters(&id).await?;
                let available_chapters: Vec<_> =
                    chapters.iter().filter(|c| c.has_content()).collect();

                if available_chapters.is_empty() {
                    println!("❌ No chapter content available for export");
                    println!(
                        "💡 Use 'quelle fetch chapters {}' to download content first",
                        id.0
                    );
                    return Ok(());
                }

                println!(
                    "  Chapters: {} available for export",
                    available_chapters.len()
                );

                let filename = format!("{}.epub", sanitize_filename(&novel.title));
                let output_path = if let Some(output_dir) = &_output {
                    format!("{}/{}", output_dir, filename)
                } else {
                    filename
                };

                println!("  Output: {}", output_path);

                // TODO: Implement actual EPUB generation using quelle_export
                println!("🚧 EPUB generation not yet implemented");
                println!("  Structure ready for export crate integration");

                println!("✅ Export prepared (implementation pending)");
            }
            None => {
                println!("❌ Novel not found: {}", id.0);
            }
        }
    } else {
        let novels = storage.list_novels(&NovelFilter::default()).await?;
        if novels.is_empty() {
            println!("📚 No novels found in library");
            return Ok(());
        }

        println!("📚 Exporting {} novels to EPUB", novels.len());
        for novel in &novels {
            let chapters = storage.list_chapters(&novel.id).await?;
            let available_chapters = chapters.iter().filter(|c| c.has_content()).count();

            if available_chapters > 0 {
                println!("  📖 {} ({} chapters)", novel.title, available_chapters);
            } else {
                println!("  ⚠️ {} (no content)", novel.title);
            }
        }

        println!("🚧 Bulk export not yet implemented");
    }
    Ok(())
}

async fn handle_export_pdf(novel_id: String, _output: Option<String>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would export to PDF: {}", novel_id);
    } else {
        println!("🚧 PDF export is not yet implemented");
        println!("📄 Novel ID: {}", novel_id);
    }
    Ok(())
}

async fn handle_export_html(
    novel_id: String,
    _output: Option<String>,
    dry_run: bool,
) -> Result<()> {
    if dry_run {
        println!("Would export to HTML: {}", novel_id);
    } else {
        println!("🚧 HTML export is not yet implemented");
        println!("🌐 Novel ID: {}", novel_id);
    }
    Ok(())
}

async fn handle_export_txt(novel_id: String, _output: Option<String>, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("Would export to TXT: {}", novel_id);
    } else {
        println!("🚧 TXT export is not yet implemented");
        println!("📝 Novel ID: {}", novel_id);
    }
    Ok(())
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>' => '_',
            c => c,
        })
        .collect()
}
