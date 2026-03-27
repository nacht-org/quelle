use async_trait::async_trait;
use bb8::Pool;
use bb8_postgres::PostgresConnectionManager;
use quelle_types::{ChapterContent, Novel};
use tokio::io::AsyncRead;
use tokio_postgres::{GenericClient, NoTls};

use crate::backends::postgres::schema::StorageConfig;
use crate::error::Result;
use crate::{
    Asset, AssetId, BookStorage, ChapterInfo, CleanupReport, NovelFilter, NovelId, NovelSummary,
};

pub mod schema;

pub struct PostgresStorage {
    pub pool: Pool<PostgresConnectionManager<NoTls>>,
    pub config: StorageConfig,
}

#[async_trait]
impl BookStorage for PostgresStorage {
    async fn store_novel(&self, novel: &Novel) -> Result<NovelId> {
        let pool = self.pool.get().await.unwrap();

        let novel_id = self
            .config
            .schema
            .insert_novel(
                pool.client(),
                &novel.url,
                &novel.title,
                novel.cover.as_deref(),
                &novel.description,
                &novel.status.as_str(),
                &novel.langs,
            )
            .await
            .unwrap();

        Ok(NovelId::new(novel_id.to_string()))
    }

    async fn get_novel(&self, id: &NovelId) -> Result<Option<Novel>> {
        todo!()
    }

    async fn update_novel(&self, id: &NovelId, novel: &Novel) -> Result<()> {
        todo!()
    }

    async fn delete_novel(&self, id: &NovelId) -> Result<bool> {
        todo!()
    }

    async fn exists_novel(&self, id: &NovelId) -> Result<bool> {
        todo!()
    }

    async fn store_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
        content: &ChapterContent,
    ) -> Result<ChapterInfo> {
        todo!()
    }

    async fn get_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<Option<ChapterContent>> {
        todo!()
    }
    async fn delete_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<Option<ChapterInfo>> {
        todo!()
    }
    async fn exists_chapter_content(
        &self,
        novel_id: &NovelId,
        volume_index: i32,
        chapter_url: &str,
    ) -> Result<bool> {
        todo!()
    }

    async fn list_novels(&self, filter: &NovelFilter) -> Result<Vec<NovelSummary>> {
        todo!()
    }

    async fn find_novel_by_url(&self, url: &str) -> Result<Option<Novel>> {
        todo!()
    }

    async fn find_novel_id_by_url(&self, url: &str) -> Result<Option<NovelId>> {
        todo!()
    }

    async fn list_chapters(&self, novel_id: &NovelId) -> Result<Vec<ChapterInfo>> {
        todo!()
    }

    async fn cleanup_dangling_data(&self) -> Result<CleanupReport> {
        todo!()
    }

    fn create_asset(&self, novel_id: NovelId, original_url: String, mime_type: String) -> Asset {
        todo!()
    }

    async fn store_asset(
        &self,
        asset: Asset,
        reader: Box<dyn AsyncRead + Send + Unpin>,
    ) -> Result<AssetId> {
        todo!()
    }

    async fn get_asset(&self, asset_id: &AssetId) -> Result<Option<Asset>> {
        todo!()
    }

    async fn get_asset_data(&self, asset_id: &AssetId) -> Result<Option<Vec<u8>>> {
        todo!()
    }

    async fn delete_asset(&self, asset_id: &AssetId) -> Result<bool> {
        todo!()
    }

    async fn find_asset_by_url(&self, url: &str) -> Result<Option<AssetId>> {
        todo!()
    }

    async fn get_novel_assets(&self, novel_id: &NovelId) -> Result<Vec<Asset>> {
        todo!()
    }
}
