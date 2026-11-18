use std::{path::Path, sync::Mutex};

use rusqlite::{Connection, params};
use zeroize::Zeroizing;

use crate::storage::migrations;

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

// Tauri and the Android widget may initialize the same database concurrently.
// Serialize opening and schema migration to avoid racing a plaintext export.
static DATABASE_OPEN_LOCK: Mutex<()> = Mutex::new(());

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path, key: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let _open_guard = DATABASE_OPEN_LOCK
            .lock()
            .map_err(|_| std::io::Error::other("Database initialization lock is poisoned."))?;
        validate_key(key)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let key_hex = Zeroizing::new(hex_key(key));

        if is_plaintext_sqlite(path)? {
            migrate_plaintext_database(path, key_hex.as_str())?;
        }

        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        apply_key(&connection, key_hex.as_str())?;
        connection.execute_batch(
            "
            PRAGMA cipher_memory_security = ON;
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = FULL;
            ",
        )?;
        migrations::migrate(&connection)?;

        Ok(Self { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

pub fn validate_key(key: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if key.len() != 32 {
        return Err("The database encryption key must contain exactly 32 bytes.".into());
    }
    Ok(())
}

fn hex_key(key: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(key.len() * 2);
    for byte in key {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn apply_key(connection: &Connection, key_hex: &str) -> Result<(), rusqlite::Error> {
    // The key is exactly 32 random bytes rendered as validated hexadecimal.
    // SQLCipher requires this PRAGMA before any other database query.
    let pragma = Zeroizing::new(format!(r#"PRAGMA key = "x'{key_hex}'";"#));
    connection.execute_batch(pragma.as_str())?;
    connection.execute_batch("PRAGMA cipher_memory_security = ON;")?;
    connection.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(())
}

fn is_plaintext_sqlite(path: &Path) -> Result<bool, std::io::Error> {
    use std::io::Read;

    if !path.is_file() {
        return Ok(false);
    }

    let mut file = std::fs::File::open(path)?;
    let mut header = [0_u8; 16];
    match file.read_exact(&mut header) {
        Ok(()) => Ok(&header == SQLITE_HEADER),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(error) => Err(error),
    }
}

fn migrate_plaintext_database(
    path: &Path,
    key_hex: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::ffi::OsString;

    let file_name = path
        .file_name()
        .ok_or("The database path has no file name.")?
        .to_string_lossy();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temporary_path = parent.join(format!(
        ".{file_name}.sqlcipher-migration-{}.tmp",
        std::process::id()
    ));

    if temporary_path.exists() {
        std::fs::remove_file(&temporary_path)?;
    }

    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        // Flush the old plaintext WAL and close the journal before exporting.
        let source = Connection::open(path)?;
        source.busy_timeout(std::time::Duration::from_secs(5))?;
        let checkpoint: (i32, i32, i32) =
            source.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
        if checkpoint.0 != 0 || (checkpoint.1 >= 0 && checkpoint.2 < checkpoint.1) {
            return Err(
                "Could not fully checkpoint the plaintext database WAL before encryption.".into(),
            );
        }

        let journal_mode: String =
            source.query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("delete") {
            return Err(
                "Could not switch the plaintext database to DELETE journaling before encryption."
                    .into(),
            );
        }

        let source_user_version: i32 =
            source.query_row("PRAGMA user_version", [], |row| row.get(0))?;

        // The path is a bound parameter. The key expression contains only
        // lowercase hex generated from the validated 32-byte random key.
        let attach = Zeroizing::new(format!(
            r#"ATTACH DATABASE ?1 AS encrypted KEY "x'{key_hex}'";"#
        ));
        source.execute(
            attach.as_str(),
            params![temporary_path.to_string_lossy().as_ref()],
        )?;
        source.execute_batch("SELECT sqlcipher_export('encrypted');")?;
        // sqlcipher_export intentionally does not copy application metadata
        // such as user_version. Preserve it or migrations may recreate tables.
        source.pragma_update(Some("encrypted"), "user_version", source_user_version)?;
        let exported_user_version: i32 =
            source.query_row("PRAGMA encrypted.user_version", [], |row| row.get(0))?;
        if exported_user_version != source_user_version {
            return Err("SQLCipher migration did not preserve the database schema version.".into());
        }
        source.execute_batch("DETACH DATABASE encrypted;")?;
        drop(source);

        // Verify both the SQLCipher page HMACs and SQLite logical structure
        // before atomically replacing the user's plaintext database.
        let encrypted = Connection::open(&temporary_path)?;
        apply_key(&encrypted, key_hex)?;

        let mut cipher_statement = encrypted.prepare("PRAGMA cipher_integrity_check")?;
        let cipher_issues = cipher_statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(cipher_statement);
        if !cipher_issues.is_empty() {
            return Err(format!(
                "SQLCipher migration failed page-integrity verification: {}",
                cipher_issues.join("; ")
            )
            .into());
        }

        let integrity: String =
            encrypted.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        let migrated_user_version: i32 =
            encrypted.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if migrated_user_version != source_user_version {
            return Err("SQLCipher migration changed the database schema version.".into());
        }
        if integrity != "ok" {
            return Err(format!(
                "SQLCipher migration failed SQLite integrity verification: {integrity}"
            )
            .into());
        }
        drop(encrypted);

        // The source is fully checkpointed and closed. Remove its old
        // sidecars before swapping files so a crash cannot leave a plaintext
        // WAL next to the newly encrypted database.
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = OsString::from(path.as_os_str());
            sidecar.push(suffix);
            let sidecar = std::path::PathBuf::from(sidecar);
            if sidecar.exists() {
                std::fs::remove_file(sidecar)?;
            }
        }

        std::fs::rename(&temporary_path, path)?;

        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = OsString::from(temporary_path.as_os_str());
            sidecar.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(sidecar));
        }
    }

    result
}
