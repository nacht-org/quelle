//! Schema configuration and database operations for the Quelle Postgres storage backend.
//!
//! `NovelSchema` describes the table and column names of a normalized Postgres
//! database layout. Because different deployments may use different naming
//! conventions, the schema is fully configurable and can be sent over the API
//! as JSON.
//!
//! Domain structs (`Novel`, `Chapter`, etc.) are returned from query methods,
//! with column mappings dynamically resolved based on the active `StorageConfig`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio_postgres::{Client, Error, Row};
use uuid::Uuid;

/// Represents a scraped novel within the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Novel {
    /// Unique identifier for the novel.
    pub id: Uuid,
    /// The original URL the novel was scraped from.
    pub url: String,
    /// The title of the novel.
    pub title: String,
    /// Optional URL or path to the novel's cover image.
    pub cover: Option<String>,
    /// Paragraphs forming the novel's description.
    pub description: Vec<String>,
    /// Publication or translation status.
    pub status: String,
    /// Languages associated with the novel.
    pub langs: Vec<String>,
}

/// Represents an author of a specific novel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    /// The ID of the novel this author is associated with.
    pub novel_id: Uuid,
    /// The name of the author.
    pub name: String,
    /// The order in which the author should be listed.
    pub position: i32,
}

/// Represents a volume or book within a novel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    /// Unique identifier for the volume.
    pub id: Uuid,
    /// The ID of the novel this volume belongs to.
    pub novel_id: Uuid,
    /// The title or name of the volume.
    pub name: String,
    /// The sequential order of this volume.
    pub index: i32,
}

/// Represents a single chapter within a volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    /// Unique identifier for the chapter.
    pub id: Uuid,
    /// The ID of the volume this chapter belongs to.
    pub volume_id: Uuid,
    /// The ID of the novel this chapter belongs to.
    pub novel_id: Uuid,
    /// The title of the chapter.
    pub title: String,
    /// The sequential order of this chapter within the volume/novel.
    pub index: i32,
    /// The original URL of the chapter.
    pub url: String,
    /// The last time this chapter was updated, if available.
    pub updated_at: Option<String>,
}

/// Represents additional dynamic metadata for a novel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    /// Unique identifier for the metadata entry.
    pub id: Uuid,
    /// The ID of the novel this metadata is associated with.
    pub novel_id: Uuid,
    /// The name of the metadata field.
    pub name: String,
    /// The primary value of the metadata field.
    pub value: String,
    /// The namespace this metadata belongs to.
    pub ns: String,
    /// Any additional arbitrary JSON data associated with this metadata.
    pub others: serde_json::Value,
}

/// Complete storage configuration sent by the client over the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Relational schema description.
    pub schema: NovelSchema,
    /// File storage backend for chapter content and assets.
    pub files: FileStorageConfig,
}

/// Where binary files (chapter content, assets) are stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileStorageConfig {
    /// Local filesystem rooted at `base_path`.
    Local { base_path: String },
}

impl FileStorageConfig {
    /// Derives the file path / object key for a chapter's content.
    pub fn chapter_path(&self, novel_id: &str, volume_index: i32, chapter_id: &str) -> String {
        match self {
            FileStorageConfig::Local { base_path } => PathBuf::from(base_path)
                .join("chapters")
                .join(novel_id)
                .join(volume_index.to_string())
                .join(chapter_id)
                .to_string_lossy()
                .into_owned(),
        }
    }

    /// Derives the file path / object key for an asset.
    pub fn asset_path(&self, novel_id: &str, asset_id: &str) -> String {
        match self {
            FileStorageConfig::Local { base_path } => PathBuf::from(base_path)
                .join("assets")
                .join(novel_id)
                .join(asset_id)
                .to_string_lossy()
                .into_owned(),
        }
    }
}

/// Configuration for the `novels` table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelsTable {
    pub table: String,
    pub id: String,
    pub url: String,
    pub title: String,
    pub cover: String,
    pub description: String,
    pub status: String,
    pub langs: String,
}

/// Configuration for the `authors` table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorsTable {
    pub table: String,
    pub novel_id: String,
    pub name: String,
    pub position: String,
}

/// Configuration for the `volumes` table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumesTable {
    pub table: String,
    pub id: String,
    pub novel_id: String,
    pub name: String,
    pub index: String,
}

/// Configuration for the `chapters` table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaptersTable {
    pub table: String,
    pub id: String,
    pub volume_id: String,
    pub novel_id: String,
    pub title: String,
    pub index: String,
    pub url: String,
    pub updated_at: String,
}

/// Configuration for the `metadata` table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataTable {
    pub table: String,
    pub id: String,
    pub novel_id: String,
    pub name: String,
    pub value: String,
    pub ns: String,
    pub others: String,
}

/// Describes the normalized Postgres schema for a Quelle deployment.
///
/// Each field names the table and its columns. No asset or chapter-content
/// table is included here — those binaries are stored as files; see
/// [`FileStorageConfig`] for path derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelSchema {
    pub novels: NovelsTable,
    pub authors: AuthorsTable,
    pub volumes: VolumesTable,
    pub chapters: ChaptersTable,
    pub metadata: MetadataTable,
}

impl Default for NovelsTable {
    fn default() -> Self {
        Self {
            table: "novels".into(),
            id: "id".into(),
            url: "url".into(),
            title: "title".into(),
            cover: "cover".into(),
            description: "description".into(),
            status: "status".into(),
            langs: "langs".into(),
        }
    }
}

impl Default for AuthorsTable {
    fn default() -> Self {
        Self {
            table: "authors".into(),
            novel_id: "novel_id".into(),
            name: "name".into(),
            position: "position".into(),
        }
    }
}

impl Default for VolumesTable {
    fn default() -> Self {
        Self {
            table: "volumes".into(),
            id: "id".into(),
            novel_id: "novel_id".into(),
            name: "name".into(),
            index: "index".into(),
        }
    }
}

impl Default for ChaptersTable {
    fn default() -> Self {
        Self {
            table: "chapters".into(),
            id: "id".into(),
            volume_id: "volume_id".into(),
            novel_id: "novel_id".into(),
            title: "title".into(),
            index: "index".into(),
            url: "url".into(),
            updated_at: "updated_at".into(),
        }
    }
}

impl Default for MetadataTable {
    fn default() -> Self {
        Self {
            table: "metadata".into(),
            id: "id".into(),
            novel_id: "novel_id".into(),
            name: "name".into(),
            value: "value".into(),
            ns: "ns".into(),
            others: "others".into(),
        }
    }
}

impl Default for NovelSchema {
    fn default() -> Self {
        Self {
            novels: NovelsTable::default(),
            authors: AuthorsTable::default(),
            volumes: VolumesTable::default(),
            chapters: ChaptersTable::default(),
            metadata: MetadataTable::default(),
        }
    }
}

impl NovelSchema {
    /// Generates the SQL statement to insert a new novel.
    pub fn sql_insert_novel(&self) -> String {
        let t = &self.novels;
        format!(
            "INSERT INTO {table} ({url}, {title}, {cover}, {description}, {status}, {langs}) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING {id}",
            table = t.table,
            id = t.id,
            url = t.url,
            title = t.title,
            cover = t.cover,
            description = t.description,
            status = t.status,
            langs = t.langs,
        )
    }

    /// Generates the SQL statement to retrieve a novel by its ID.
    pub fn sql_get_novel_by_id(&self) -> String {
        format!(
            "SELECT * FROM {table} WHERE {id} = $1",
            table = self.novels.table,
            id = self.novels.id
        )
    }

    /// Generates the SQL statement to retrieve a novel by its URL.
    pub fn sql_get_novel_by_url(&self) -> String {
        format!(
            "SELECT * FROM {table} WHERE {url} = $1",
            table = self.novels.table,
            url = self.novels.url
        )
    }

    /// Generates the SQL statement to retrieve a novel's ID by its URL.
    pub fn sql_get_novel_id_by_url(&self) -> String {
        format!(
            "SELECT {id} FROM {table} WHERE {url} = $1",
            table = self.novels.table,
            id = self.novels.id,
            url = self.novels.url
        )
    }

    /// Generates the SQL statement to update an existing novel.
    pub fn sql_update_novel(&self) -> String {
        let t = &self.novels;
        format!(
            "UPDATE {table} SET {url} = $1, {title} = $2, {cover} = $3, \
             {description} = $4, {status} = $5, {langs} = $6 WHERE {id} = $7",
            table = t.table,
            id = t.id,
            url = t.url,
            title = t.title,
            cover = t.cover,
            description = t.description,
            status = t.status,
            langs = t.langs,
        )
    }

    /// Generates the SQL statement to delete a novel by its ID.
    pub fn sql_delete_novel(&self) -> String {
        format!(
            "DELETE FROM {table} WHERE {id} = $1",
            table = self.novels.table,
            id = self.novels.id
        )
    }

    /// Generates the SQL statement to list novels, optionally filtered by status.
    pub fn sql_list_novels(&self, filter_by_status: bool) -> String {
        let t = &self.novels;
        if filter_by_status {
            format!(
                "SELECT * FROM {table} WHERE {status} = $1 ORDER BY {title}",
                table = t.table,
                status = t.status,
                title = t.title
            )
        } else {
            format!(
                "SELECT * FROM {table} ORDER BY {title}",
                table = t.table,
                title = t.title
            )
        }
    }

    /// Generates the SQL statement to insert an author for a novel.
    pub fn sql_insert_author(&self) -> String {
        let t = &self.authors;
        format!(
            "INSERT INTO {table} ({novel_id}, {name}, {position}) VALUES ($1, $2, $3)",
            table = t.table,
            novel_id = t.novel_id,
            name = t.name,
            position = t.position
        )
    }

    /// Generates the SQL statement to retrieve all authors for a specific novel.
    pub fn sql_get_authors(&self) -> String {
        let t = &self.authors;
        format!(
            "SELECT * FROM {table} WHERE {novel_id} = $1 ORDER BY {position}",
            table = t.table,
            novel_id = t.novel_id,
            position = t.position
        )
    }

    /// Generates the SQL statement to delete all authors associated with a novel.
    pub fn sql_delete_authors(&self) -> String {
        format!(
            "DELETE FROM {table} WHERE {novel_id} = $1",
            table = self.authors.table,
            novel_id = self.authors.novel_id
        )
    }

    /// Generates the SQL statement to insert a volume for a novel.
    pub fn sql_insert_volume(&self) -> String {
        let t = &self.volumes;
        format!(
            "INSERT INTO {table} ({novel_id}, {name}, {index}) VALUES ($1, $2, $3) RETURNING {id}",
            table = t.table,
            id = t.id,
            novel_id = t.novel_id,
            name = t.name,
            index = t.index
        )
    }

    /// Generates the SQL statement to retrieve all volumes for a specific novel.
    pub fn sql_get_volumes(&self) -> String {
        let t = &self.volumes;
        format!(
            "SELECT * FROM {table} WHERE {novel_id} = $1 ORDER BY {index}",
            table = t.table,
            novel_id = t.novel_id,
            index = t.index
        )
    }

    /// Generates the SQL statement to delete all volumes associated with a novel.
    pub fn sql_delete_volumes(&self) -> String {
        format!(
            "DELETE FROM {table} WHERE {novel_id} = $1",
            table = self.volumes.table,
            novel_id = self.volumes.novel_id
        )
    }

    /// Generates the SQL statement to insert a chapter.
    pub fn sql_insert_chapter(&self) -> String {
        let t = &self.chapters;
        format!(
            "INSERT INTO {table} ({volume_id}, {novel_id}, {title}, {index}, {url}, {updated_at}) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING {id}",
            table = t.table,
            id = t.id,
            volume_id = t.volume_id,
            novel_id = t.novel_id,
            title = t.title,
            index = t.index,
            url = t.url,
            updated_at = t.updated_at
        )
    }

    /// Generates the SQL statement to retrieve all chapters for a specific novel.
    pub fn sql_get_chapters_by_novel(&self) -> String {
        format!(
            "SELECT * FROM {table} WHERE {novel_id} = $1 ORDER BY {index}",
            table = self.chapters.table,
            novel_id = self.chapters.novel_id,
            index = self.chapters.index
        )
    }

    /// Generates the SQL statement to retrieve all chapters within a specific volume.
    pub fn sql_get_chapters_by_volume(&self) -> String {
        format!(
            "SELECT * FROM {table} WHERE {volume_id} = $1 ORDER BY {index}",
            table = self.chapters.table,
            volume_id = self.chapters.volume_id,
            index = self.chapters.index
        )
    }

    /// Generates the SQL statement to retrieve a chapter's ID by its URL.
    pub fn sql_get_chapter_id_by_url(&self) -> String {
        format!(
            "SELECT {id} FROM {table} WHERE {novel_id} = $1 AND {url} = $2",
            table = self.chapters.table,
            id = self.chapters.id,
            novel_id = self.chapters.novel_id,
            url = self.chapters.url
        )
    }

    /// Generates the SQL statement to delete all chapters associated with a novel.
    pub fn sql_delete_chapters_by_novel(&self) -> String {
        format!(
            "DELETE FROM {table} WHERE {novel_id} = $1",
            table = self.chapters.table,
            novel_id = self.chapters.novel_id
        )
    }

    /// Generates the SQL statement to insert metadata for a novel.
    pub fn sql_insert_metadata(&self) -> String {
        let t = &self.metadata;
        format!(
            "INSERT INTO {table} ({novel_id}, {name}, {value}, {ns}, {others}) VALUES ($1, $2, $3, $4, $5)",
            table = t.table,
            novel_id = t.novel_id,
            name = t.name,
            value = t.value,
            ns = t.ns,
            others = t.others
        )
    }

    /// Generates the SQL statement to retrieve all metadata for a specific novel.
    pub fn sql_get_metadata(&self) -> String {
        format!(
            "SELECT * FROM {table} WHERE {novel_id} = $1",
            table = self.metadata.table,
            novel_id = self.metadata.novel_id
        )
    }

    /// Generates the SQL statement to delete all metadata associated with a novel.
    pub fn sql_delete_metadata(&self) -> String {
        format!(
            "DELETE FROM {table} WHERE {novel_id} = $1",
            table = self.metadata.table,
            novel_id = self.metadata.novel_id
        )
    }

    /// Generates `CREATE TABLE IF NOT EXISTS` DDL statements for all configured tables.
    pub fn sql_create_tables(&self) -> String {
        let n = &self.novels;
        let a = &self.authors;
        let v = &self.volumes;
        let c = &self.chapters;
        let m = &self.metadata;

        format!(
            r#"
CREATE TABLE IF NOT EXISTS {novels} (
    {n_id}          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    {n_url}         TEXT NOT NULL UNIQUE,
    {n_title}       TEXT NOT NULL,
    {n_cover}       TEXT,
    {n_description} TEXT[] NOT NULL DEFAULT '{{}}',
    {n_status}      TEXT NOT NULL,
    {n_langs}       TEXT[] NOT NULL DEFAULT '{{}}'
);

CREATE TABLE IF NOT EXISTS {authors} (
    {a_novel_id}  UUID NOT NULL REFERENCES {novels}({n_id}) ON DELETE CASCADE,
    {a_name}      TEXT NOT NULL,
    {a_position}  INTEGER NOT NULL,
    PRIMARY KEY ({a_novel_id}, {a_position})
);

CREATE TABLE IF NOT EXISTS {volumes} (
    {v_id}       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    {v_novel_id} UUID NOT NULL REFERENCES {novels}({n_id}) ON DELETE CASCADE,
    {v_name}     TEXT NOT NULL,
    {v_index}    INTEGER NOT NULL,
    UNIQUE ({v_novel_id}, {v_index})
);

CREATE TABLE IF NOT EXISTS {chapters} (
    {c_id}         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    {c_volume_id}  UUID NOT NULL REFERENCES {volumes}({v_id}) ON DELETE CASCADE,
    {c_novel_id}   UUID NOT NULL REFERENCES {novels}({n_id}) ON DELETE CASCADE,
    {c_title}      TEXT NOT NULL,
    {c_index}      INTEGER NOT NULL,
    {c_url}        TEXT NOT NULL,
    {c_updated_at} TEXT,
    UNIQUE ({c_novel_id}, {c_url})
);

CREATE TABLE IF NOT EXISTS {metadata} (
    {m_id}       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    {m_novel_id} UUID NOT NULL REFERENCES {novels}({n_id}) ON DELETE CASCADE,
    {m_name}     TEXT NOT NULL,
    {m_value}    TEXT NOT NULL,
    {m_ns}       TEXT NOT NULL,
    {m_others}   JSONB NOT NULL DEFAULT '[]'
);
"#,
            novels = n.table,
            n_id = n.id,
            n_url = n.url,
            n_title = n.title,
            n_cover = n.cover,
            n_description = n.description,
            n_status = n.status,
            n_langs = n.langs,
            authors = a.table,
            a_novel_id = a.novel_id,
            a_name = a.name,
            a_position = a.position,
            volumes = v.table,
            v_id = v.id,
            v_novel_id = v.novel_id,
            v_name = v.name,
            v_index = v.index,
            chapters = c.table,
            c_id = c.id,
            c_volume_id = c.volume_id,
            c_novel_id = c.novel_id,
            c_title = c.title,
            c_index = c.index,
            c_url = c.url,
            c_updated_at = c.updated_at,
            metadata = m.table,
            m_id = m.id,
            m_novel_id = m.novel_id,
            m_name = m.name,
            m_value = m.value,
            m_ns = m.ns,
            m_others = m.others,
        )
    }
}

impl NovelSchema {
    /// Parses a database row into a `Novel` struct using the configured column names.
    fn parse_novel(&self, row: &Row) -> Result<Novel, Error> {
        let t = &self.novels;
        Ok(Novel {
            id: row.try_get(t.id.as_str())?,
            url: row.try_get(t.url.as_str())?,
            title: row.try_get(t.title.as_str())?,
            cover: row.try_get(t.cover.as_str())?,
            description: row.try_get(t.description.as_str())?,
            status: row.try_get(t.status.as_str())?,
            langs: row.try_get(t.langs.as_str())?,
        })
    }

    /// Parses a database row into an `Author` struct using the configured column names.
    fn parse_author(&self, row: &Row) -> Result<Author, Error> {
        let t = &self.authors;
        Ok(Author {
            novel_id: row.try_get(t.novel_id.as_str())?,
            name: row.try_get(t.name.as_str())?,
            position: row.try_get(t.position.as_str())?,
        })
    }

    /// Parses a database row into a `Volume` struct using the configured column names.
    fn parse_volume(&self, row: &Row) -> Result<Volume, Error> {
        let t = &self.volumes;
        Ok(Volume {
            id: row.try_get(t.id.as_str())?,
            novel_id: row.try_get(t.novel_id.as_str())?,
            name: row.try_get(t.name.as_str())?,
            index: row.try_get(t.index.as_str())?,
        })
    }

    /// Parses a database row into a `Chapter` struct using the configured column names.
    fn parse_chapter(&self, row: &Row) -> Result<Chapter, Error> {
        let t = &self.chapters;
        Ok(Chapter {
            id: row.try_get(t.id.as_str())?,
            volume_id: row.try_get(t.volume_id.as_str())?,
            novel_id: row.try_get(t.novel_id.as_str())?,
            title: row.try_get(t.title.as_str())?,
            index: row.try_get(t.index.as_str())?,
            url: row.try_get(t.url.as_str())?,
            updated_at: row.try_get(t.updated_at.as_str())?,
        })
    }

    /// Parses a database row into a `Metadata` struct using the configured column names.
    fn parse_metadata(&self, row: &Row) -> Result<Metadata, Error> {
        let t = &self.metadata;
        Ok(Metadata {
            id: row.try_get(t.id.as_str())?,
            novel_id: row.try_get(t.novel_id.as_str())?,
            name: row.try_get(t.name.as_str())?,
            value: row.try_get(t.value.as_str())?,
            ns: row.try_get(t.ns.as_str())?,
            others: row.try_get(t.others.as_str())?, // requires tokio-postgres `with-serde_json-1` feature
        })
    }
}

impl NovelSchema {
    /// Executes the `CREATE TABLE IF NOT EXISTS` statements for all tables.
    pub async fn create_tables(&self, client: &Client) -> Result<(), Error> {
        client.batch_execute(&self.sql_create_tables()).await
    }

    /// Inserts a new novel into the database and returns its assigned ID.
    pub async fn insert_novel(
        &self,
        client: &Client,
        url: &str,
        title: &str,
        cover: Option<&str>,
        description: &[String],
        status: &str,
        langs: &[String],
    ) -> Result<Uuid, Error> {
        let row = client
            .query_one(
                &self.sql_insert_novel(),
                &[&url, &title, &cover, &description, &status, &langs],
            )
            .await?;
        row.try_get(self.novels.id.as_str())
    }

    /// Retrieves a single novel by its unique ID.
    pub async fn get_novel_by_id(
        &self,
        client: &Client,
        id: &Uuid,
    ) -> Result<Option<Novel>, Error> {
        let row_opt = client.query_opt(&self.sql_get_novel_by_id(), &[id]).await?;
        row_opt.map(|r| self.parse_novel(&r)).transpose()
    }

    /// Retrieves a single novel by its source URL.
    pub async fn get_novel_by_url(
        &self,
        client: &Client,
        url: &str,
    ) -> Result<Option<Novel>, Error> {
        let row_opt = client
            .query_opt(&self.sql_get_novel_by_url(), &[&url])
            .await?;
        row_opt.map(|r| self.parse_novel(&r)).transpose()
    }

    /// Retrieves only the ID of a novel by its source URL.
    pub async fn get_novel_id_by_url(
        &self,
        client: &Client,
        url: &str,
    ) -> Result<Option<Uuid>, Error> {
        let row_opt = client
            .query_opt(&self.sql_get_novel_id_by_url(), &[&url])
            .await?;
        match row_opt {
            Some(row) => Ok(Some(row.try_get(self.novels.id.as_str())?)),
            None => Ok(None),
        }
    }

    /// Updates the core information of an existing novel.
    pub async fn update_novel(
        &self,
        client: &Client,
        id: &Uuid,
        url: &str,
        title: &str,
        cover: Option<&str>,
        description: &[String],
        status: &str,
        langs: &[String],
    ) -> Result<u64, Error> {
        client
            .execute(
                &self.sql_update_novel(),
                &[&url, &title, &cover, &description, &status, &langs, id],
            )
            .await
    }

    /// Deletes a novel by its ID. Cascading constraints should automatically clean up relations.
    pub async fn delete_novel(&self, client: &Client, id: &Uuid) -> Result<u64, Error> {
        client.execute(&self.sql_delete_novel(), &[id]).await
    }

    /// Lists all novels currently tracked in the database, ordered by title.
    pub async fn list_all_novels(&self, client: &Client) -> Result<Vec<Novel>, Error> {
        let rows = client.query(&self.sql_list_novels(false), &[]).await?;
        rows.iter().map(|r| self.parse_novel(r)).collect()
    }

    /// Lists novels filtered by a specific status, ordered by title.
    pub async fn list_novels_by_status(
        &self,
        client: &Client,
        status: &str,
    ) -> Result<Vec<Novel>, Error> {
        let rows = client
            .query(&self.sql_list_novels(true), &[&status])
            .await?;
        rows.iter().map(|r| self.parse_novel(r)).collect()
    }

    /// Inserts an author record associated with a novel.
    pub async fn insert_author(
        &self,
        client: &Client,
        novel_id: &Uuid,
        name: &str,
        position: i32,
    ) -> Result<u64, Error> {
        client
            .execute(&self.sql_insert_author(), &[novel_id, &name, &position])
            .await
    }

    /// Retrieves all authors associated with a novel, ordered by their position.
    pub async fn get_authors(
        &self,
        client: &Client,
        novel_id: &Uuid,
    ) -> Result<Vec<Author>, Error> {
        let rows = client.query(&self.sql_get_authors(), &[novel_id]).await?;
        rows.iter().map(|r| self.parse_author(r)).collect()
    }

    /// Deletes all author records associated with a specific novel.
    pub async fn delete_authors(&self, client: &Client, novel_id: &Uuid) -> Result<u64, Error> {
        client
            .execute(&self.sql_delete_authors(), &[novel_id])
            .await
    }

    /// Inserts a new volume for a novel and returns its assigned ID.
    pub async fn insert_volume(
        &self,
        client: &Client,
        novel_id: &Uuid,
        name: &str,
        index: i32,
    ) -> Result<Uuid, Error> {
        let row = client
            .query_one(&self.sql_insert_volume(), &[novel_id, &name, &index])
            .await?;
        row.try_get(self.volumes.id.as_str())
    }

    /// Retrieves all volumes belonging to a specific novel, ordered by index.
    pub async fn get_volumes(
        &self,
        client: &Client,
        novel_id: &Uuid,
    ) -> Result<Vec<Volume>, Error> {
        let rows = client.query(&self.sql_get_volumes(), &[novel_id]).await?;
        rows.iter().map(|r| self.parse_volume(r)).collect()
    }

    /// Deletes all volume records associated with a specific novel.
    pub async fn delete_volumes(&self, client: &Client, novel_id: &Uuid) -> Result<u64, Error> {
        client
            .execute(&self.sql_delete_volumes(), &[novel_id])
            .await
    }

    /// Inserts a new chapter and returns its assigned ID.
    pub async fn insert_chapter(
        &self,
        client: &Client,
        volume_id: &Uuid,
        novel_id: &Uuid,
        title: &str,
        index: i32,
        url: &str,
        updated_at: Option<&str>,
    ) -> Result<Uuid, Error> {
        let row = client
            .query_one(
                &self.sql_insert_chapter(),
                &[volume_id, novel_id, &title, &index, &url, &updated_at],
            )
            .await?;
        row.try_get(self.chapters.id.as_str())
    }

    /// Retrieves all chapters for a given novel, ordered by index.
    pub async fn get_chapters_by_novel(
        &self,
        client: &Client,
        novel_id: &Uuid,
    ) -> Result<Vec<Chapter>, Error> {
        let rows = client
            .query(&self.sql_get_chapters_by_novel(), &[novel_id])
            .await?;
        rows.iter().map(|r| self.parse_chapter(r)).collect()
    }

    /// Retrieves all chapters belonging to a specific volume, ordered by index.
    pub async fn get_chapters_by_volume(
        &self,
        client: &Client,
        volume_id: &Uuid,
    ) -> Result<Vec<Chapter>, Error> {
        let rows = client
            .query(&self.sql_get_chapters_by_volume(), &[volume_id])
            .await?;
        rows.iter().map(|r| self.parse_chapter(r)).collect()
    }

    /// Retrieves the ID of a chapter matching both the novel's ID and the chapter's source URL.
    pub async fn get_chapter_id_by_url(
        &self,
        client: &Client,
        novel_id: &Uuid,
        url: &str,
    ) -> Result<Option<Uuid>, Error> {
        let row_opt = client
            .query_opt(&self.sql_get_chapter_id_by_url(), &[novel_id, &url])
            .await?;
        match row_opt {
            Some(row) => Ok(Some(row.try_get(self.chapters.id.as_str())?)),
            None => Ok(None),
        }
    }

    /// Deletes all chapters associated with a specific novel.
    pub async fn delete_chapters_by_novel(
        &self,
        client: &Client,
        novel_id: &Uuid,
    ) -> Result<u64, Error> {
        client
            .execute(&self.sql_delete_chapters_by_novel(), &[novel_id])
            .await
    }

    /// Inserts metadata associated with a novel.
    pub async fn insert_metadata(
        &self,
        client: &Client,
        novel_id: &Uuid,
        name: &str,
        value: &str,
        ns: &str,
        others: &serde_json::Value,
    ) -> Result<u64, Error> {
        client
            .execute(
                &self.sql_insert_metadata(),
                &[
                    novel_id,
                    &name,
                    &value,
                    &ns,
                    &tokio_postgres::types::Json(others),
                ],
            )
            .await
    }

    /// Retrieves all metadata entries linked to a given novel.
    pub async fn get_metadata(
        &self,
        client: &Client,
        novel_id: &Uuid,
    ) -> Result<Vec<Metadata>, Error> {
        let rows = client.query(&self.sql_get_metadata(), &[novel_id]).await?;
        rows.iter().map(|r| self.parse_metadata(r)).collect()
    }

    /// Deletes all metadata associated with a specific novel.
    pub async fn delete_metadata(&self, client: &Client, novel_id: &Uuid) -> Result<u64, Error> {
        client
            .execute(&self.sql_delete_metadata(), &[novel_id])
            .await
    }
}
