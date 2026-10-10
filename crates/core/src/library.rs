use crate::image::ImageInfo;
use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootRecord {
    pub id: i64,
    pub path: PathBuf,
    pub origin: String,
    pub product: String,
    pub patched_with: String,
    pub source_sha1: String,
    pub created: String,
    pub info: ImageInfo,
    pub lineage: Option<ArtifactLineage>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactLineage {
    pub stock_sha256: String,
    pub firmware_origin: String,
    pub provider: crate::root::Solution,
    pub version: String,
    pub method: String,
}
pub struct Library {
    connection: Connection,
}
impl Library {
    pub fn open(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let connection = Connection::open(directory.join("library.db"))?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        anyhow::ensure!(
            version <= 2,
            "Library schema is newer than this application"
        );
        if version == 0 {
            let backup = directory.join(format!(
                "library-pre-v1-{}.db",
                chrono::Utc::now().timestamp_millis()
            ));
            let tables: u32 = connection.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )?;
            if tables > 0 {
                connection.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
            }
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE IF NOT EXISTS journal(id TEXT PRIMARY KEY,title TEXT NOT NULL,digest TEXT NOT NULL,status TEXT NOT NULL,updated TEXT NOT NULL,verification TEXT NOT NULL); CREATE TABLE IF NOT EXISTS review_queue(id TEXT PRIMARY KEY,intent TEXT NOT NULL,created TEXT NOT NULL,status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS images(id INTEGER PRIMARY KEY,path TEXT NOT NULL,sha256 TEXT NOT NULL UNIQUE,origin TEXT NOT NULL,product TEXT NOT NULL,patched_with TEXT NOT NULL,source_sha1 TEXT NOT NULL,created TEXT NOT NULL,info TEXT NOT NULL); CREATE TABLE IF NOT EXISTS operations(id INTEGER PRIMARY KEY,created TEXT NOT NULL,kind TEXT NOT NULL,status TEXT NOT NULL,details TEXT NOT NULL); PRAGMA user_version=1; COMMIT;")?;
        }
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS images(id INTEGER PRIMARY KEY,path TEXT NOT NULL,sha256 TEXT NOT NULL UNIQUE,origin TEXT NOT NULL,product TEXT NOT NULL,patched_with TEXT NOT NULL,source_sha1 TEXT NOT NULL,created TEXT NOT NULL,info TEXT NOT NULL); CREATE TABLE IF NOT EXISTS operations(id INTEGER PRIMARY KEY,created TEXT NOT NULL,kind TEXT NOT NULL,status TEXT NOT NULL,details TEXT NOT NULL);")?;
        if version < 2 {
            if version == 1 {
                let backup = directory.join(format!(
                    "library-pre-v2-{}.db",
                    chrono::Utc::now().timestamp_millis()
                ));
                connection.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])?;
            }
            connection.execute_batch("BEGIN IMMEDIATE;CREATE TABLE IF NOT EXISTS lineage(sha256 TEXT PRIMARY KEY,metadata TEXT NOT NULL);PRAGMA user_version=2;COMMIT;")?;
        }
        Ok(Self { connection })
    }
    pub fn insert(
        &self,
        path: &Path,
        origin: &str,
        product: &str,
        patched_with: &str,
        source_sha1: &str,
        info: &ImageInfo,
    ) -> Result<()> {
        self.connection.execute("INSERT INTO images(path,sha256,origin,product,patched_with,source_sha1,created,info) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(sha256) DO UPDATE SET path=excluded.path",params![path.to_string_lossy(),info.sha256,origin,product,patched_with,source_sha1,chrono::Utc::now().to_rfc3339(),serde_json::to_string(info)?])?;
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<BootRecord>> {
        let mut stmt=self.connection.prepare("SELECT id,path,origin,product,patched_with,source_sha1,created,info,metadata FROM images LEFT JOIN lineage USING(sha256) ORDER BY id DESC")?;
        let rows = stmt.query_map([], |row| {
            let info: String = row.get(7)?;
            let parsed = serde_json::from_str(&info).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(BootRecord {
                id: row.get(0)?,
                path: PathBuf::from(row.get::<_, String>(1)?),
                origin: row.get(2)?,
                product: row.get(3)?,
                patched_with: row.get(4)?,
                source_sha1: row.get(5)?,
                created: row.get(6)?,
                info: parsed,
                lineage: row
                    .get::<_, Option<String>>(8)?
                    .map(|s| serde_json::from_str(&s))
                    .transpose()
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn remove(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM images WHERE id=?1", [id])?;
        Ok(())
    }
    pub fn set_lineage(&self, sha256: &str, lineage: &ArtifactLineage) -> Result<()> {
        self.connection.execute("INSERT INTO lineage VALUES(?1,?2) ON CONFLICT(sha256) DO UPDATE SET metadata=excluded.metadata",params![sha256,serde_json::to_string(lineage)?])?;
        Ok(())
    }
    pub fn record_operation(&self, kind: &str, status: &str, details: &str) -> Result<()> {
        self.connection.execute(
            "INSERT INTO operations(created,kind,status,details) VALUES(?1,?2,?3,?4)",
            params![chrono::Utc::now().to_rfc3339(), kind, status, details],
        )?;
        Ok(())
    }
    pub fn import_legacy(&self, path: &Path) -> Result<(usize, Vec<String>)> {
        let source = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut columns = source.prepare("PRAGMA table_info(BOOT)")?;
        let names = columns
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        anyhow::ensure!(
            names.iter().any(|n| n == "file_path") && names.iter().any(|n| n == "boot_hash"),
            "Not a supported PixelFlasher BOOT database"
        );
        let patch = if names.iter().any(|n| n == "patch_method") {
            "COALESCE(patch_method,magisk_version,'patched')"
        } else {
            "COALESCE(magisk_version,'patched')"
        };
        let mut statement=source.prepare(&format!("SELECT file_path,boot_hash,COALESCE(hardware,''),COALESCE(is_patched,0),{patch} FROM BOOT"))?;
        let rows = statement.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i32>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let (mut imported, mut skipped) = (0, vec![]);
        for row in rows {
            let (file, hash, product, patched, method) = row?;
            let image_path = PathBuf::from(&file);
            match crate::image::inspect(&image_path) {
                Ok(info) => {
                    if !info.sha1.eq_ignore_ascii_case(&hash) {
                        skipped.push(format!("Hash mismatch: {file}"));
                        continue;
                    }
                    self.insert(
                        &image_path,
                        &format!("Legacy database {}", path.display()),
                        &product,
                        if patched != 0 { &method } else { "" },
                        "",
                        &info,
                    )?;
                    imported += 1;
                }
                Err(e) => skipped.push(format!("{file}: {e}")),
            }
        }
        Ok((imported, skipped))
    }
    pub fn journal(
        &self,
        id: &str,
        title: &str,
        digest: &str,
        status: &str,
        verification: &str,
    ) -> Result<()> {
        self.connection.execute("INSERT INTO journal VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET status=excluded.status,updated=excluded.updated,verification=excluded.verification",params![id,title,digest,status,chrono::Utc::now().to_rfc3339(),verification])?;
        Ok(())
    }
    pub fn recent_operations(&self) -> Result<Vec<crate::operation::OperationRecord>> {
        let mut statement=self.connection.prepare("SELECT id,title,status,updated,verification FROM journal ORDER BY updated DESC LIMIT 50")?;
        Ok(statement
            .query_map([], |r| {
                Ok(crate::operation::OperationRecord {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    status: r.get(2)?,
                    updated: r.get(3)?,
                    verification: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }
    pub fn recover_interrupted(&self) -> Result<usize> {
        Ok(self.connection.execute("UPDATE journal SET status='Interrupted',verification='Previous process ended; fresh inspection and review required' WHERE status='Running'",[])?)
    }
    pub fn enqueue_review(&self, intent: &crate::operation::OperationIntent) -> Result<String> {
        let id = crate::operation::opaque_id("review");
        self.connection.execute(
            "INSERT INTO review_queue VALUES(?1,?2,?3,'Pending')",
            params![
                id,
                serde_json::to_string(intent)?,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(id)
    }
    pub fn review_requests(&self) -> Result<Vec<(String, crate::operation::OperationIntent)>> {
        let mut statement = self.connection.prepare(
            "SELECT id,intent FROM review_queue WHERE status='Pending' ORDER BY created",
        )?;
        let rows =
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = vec![];
        for row in rows {
            let (id, text) = row?;
            out.push((id, serde_json::from_str(&text)?));
        }
        Ok(out)
    }
    pub fn dismiss_review(&self, id: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE review_queue SET status='Dismissed' WHERE id=?1",
            [id],
        )?;
        Ok(())
    }
}
