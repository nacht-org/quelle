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

    println!("🚧 EPUB export is not yet fully implemented");
    println!("📚 Novel ID: {}", novel_id);

    // Check if novel exists
    if novel_id != "all" {
        let id = NovelId::new(novel_id.clone());
        match storage.get_novel(&id).await? {
            Some(novel) => {
                println!("💡 Would export: {}", novel.title);
                println!("  With cover image (if available)");
                println!("  With all downloaded chapters");
                println!(
                    "  To current directory as {}.epub",
                    sanitize_filename(&novel.title)
                );
            }
            None => {
                println!("❌ Novel not found: {}", id.as_str());
            }
        }
    } else {
        let novels = storage.list_novels(&NovelFilter::default()).await?;
        println!("💡 Would export {} novels to EPUB", novels.len());
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
