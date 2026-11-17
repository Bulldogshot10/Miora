use std::path::Path;

use rusqlite::{Connection, params};
use zeroize::Zeroizing;

use crate::storage::migrations;

const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path, key: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        validate_key(key)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let key_hex = Zeroizing::new(hex_key(key));

        if is_plaintext_sqlite(path)? {
            migrate_plaintext_database(path, key_hex.as_str())?;
        }

        let connection = Connection::open(path)?;
        apply_key(&connection, key_hex.as_str())?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
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
    connection.execute_batch(&format!(r#"PRAGMA key = "x'{key_hex}'";"#))?;
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
        source.execute_batch(
            "
            PRAGMA wal_checkpoint(TRUNCATE);
            PRAGMA journal_mode = DELETE;
            ",
        )?;

        // The path is a bound parameter. The key expression contains only
        // lowercase hex generated from the validated 32-byte random key.
        let attach = format!(r#"ATTACH DATABASE ?1 AS encrypted KEY "x'{key_hex}'";"#);
        source.execute(&attach, params![temporary_path.to_string_lossy().as_ref()])?;
        source.execute_batch("SELECT sqlcipher_export('encrypted');")?;
        source.execute_batch("DETACH DATABASE encrypted;")?;
        drop(source);

        // Verify both the SQLCipher page HMACs and SQLite logical structure
        // before atomically replacing the user's plaintext database.
        let encrypted = Connection::open(&temporary_path)?;
        apply_key(&encrypted, key_hex)?;

        let cipher_issues = encrypted
            .prepare("PRAGMA cipher_integrity_check")?
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if !cipher_issues.is_empty() {
            return Err(format!(
                "SQLCipher migration failed page-integrity verification: {}",
                cipher_issues.join("; ")
            )
            .into());
        }

        let integrity: String =
            encrypted.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(format!(
                "SQLCipher migration failed SQLite integrity verification: {integrity}"
            )
            .into());
        }
        drop(encrypted);

        std::fs::rename(&temporary_path, path)?;

        for suffix in ["-wal", "-shm"] {
            let mut sidecar = OsString::from(path.as_os_str());
            sidecar.push(suffix);
            let sidecar = std::path::PathBuf::from(sidecar);
            if sidecar.exists() {
                std::fs::remove_file(sidecar)?;
            }
        }

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
