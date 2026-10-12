//! Transaction Management
//!
//! Provides atomic package operations with rollback capability.

extern crate alloc;

use crate::database::Database;
use crate::io;
use alloc::string::String;
use alloc::vec::Vec;

/// Transaction operation types
#[derive(Clone)]
pub enum Operation {
    /// Install a package
    Install {
        name: String,
        version: String,
        files: Vec<String>,
    },
    /// Remove a package
    Remove {
        name: String,
        version: String,
        files: Vec<String>,
    },
    /// Upgrade a package (remove old, install new)
    Upgrade {
        name: String,
        old_version: String,
        new_version: String,
        old_files: Vec<String>,
        new_files: Vec<String>,
    },
}

/// Transaction state
#[derive(Clone, Copy, PartialEq)]
pub enum TransactionState {
    /// Transaction not started
    Pending,
    /// Transaction in progress
    InProgress,
    /// Transaction completed successfully
    Completed,
    /// Transaction failed, rolled back
    RolledBack,
}

/// A transaction for atomic package operations
pub struct Transaction {
    /// Operations to perform
    operations: Vec<Operation>,
    /// Current state
    state: TransactionState,
    /// Completed operations (for rollback)
    completed: Vec<CompletedOp>,
    /// Backup directory for rollback
    backup_dir: Vec<u8>,
}

/// Record of a completed operation (for rollback)
#[derive(Clone)]
struct CompletedOp {
    op_index: usize,
    files_created: Vec<String>,
    files_removed: Vec<String>,
    db_changes: Vec<DbChange>,
}

#[derive(Clone)]
enum DbChange {
    PackageAdded(String),
    PackageRemoved(String),
    FilesRegistered(Vec<String>),
    FilesUnregistered(Vec<String>),
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

impl Transaction {
    /// Create a new transaction
    pub fn new() -> Self {
        let mut backup_dir = Vec::from(b"/var/lib/abp/backup.".as_slice());
        // Add timestamp for uniqueness
        let time = super::database::current_time();
        let mut num_buf = [0u8; 20];
        let num_str = format_u64(time, &mut num_buf);
        backup_dir.extend_from_slice(num_str);

        Transaction {
            operations: Vec::new(),
            state: TransactionState::Pending,
            completed: Vec::new(),
            backup_dir,
        }
    }

    /// Add an install operation
    pub fn add_install(&mut self, name: String, version: String, files: Vec<String>) {
        self.operations.push(Operation::Install {
            name,
            version,
            files,
        });
    }

    /// Add a remove operation
    pub fn add_remove(&mut self, name: String, version: String, files: Vec<String>) {
        self.operations.push(Operation::Remove {
            name,
            version,
            files,
        });
    }

    /// Add an upgrade operation
    pub fn add_upgrade(
        &mut self,
        name: String,
        old_version: String,
        new_version: String,
        old_files: Vec<String>,
        new_files: Vec<String>,
    ) {
        self.operations.push(Operation::Upgrade {
            name,
            old_version,
            new_version,
            old_files,
            new_files,
        });
    }

    /// Execute the transaction
    pub fn execute(&mut self, db: &Database) -> Result<(), String> {
        if self.state != TransactionState::Pending {
            return Err(String::from("transaction already executed"));
        }

        self.state = TransactionState::InProgress;

        // Create backup directory
        io::mkdir(&self.backup_dir, 0o755);

        // Execute each operation
        for (i, op) in self.operations.iter().enumerate() {
            match self.execute_operation(db, i, op.clone()) {
                Ok(completed) => {
                    self.completed.push(completed);
                }
                Err(e) => {
                    // Rollback on failure
                    self.rollback(db);
                    return Err(e);
                }
            }
        }

        // Clean up backup directory
        self.cleanup_backup();

        self.state = TransactionState::Completed;
        Ok(())
    }

    /// Execute a single operation
    fn execute_operation(
        &self,
        db: &Database,
        index: usize,
        op: Operation,
    ) -> Result<CompletedOp, String> {
        match op {
            Operation::Install {
                name,
                version,
                files,
            } => self.execute_install(db, index, &name, &version, &files),
            Operation::Remove {
                name,
                version,
                files,
            } => self.execute_remove(db, index, &name, &version, &files),
            Operation::Upgrade {
                name,
                old_version,
                new_version,
                old_files,
                new_files,
            } => self.execute_upgrade(
                db,
                index,
                &name,
                &old_version,
                &new_version,
                &old_files,
                &new_files,
            ),
        }
    }

    fn execute_install(
        &self,
        _db: &Database,
        index: usize,
        _name: &str,
        _version: &str,
        files: &[String],
    ) -> Result<CompletedOp, String> {
        // For install, files are created by the caller (install.rs)
        // We just track them for rollback purposes
        Ok(CompletedOp {
            op_index: index,
            files_created: files.to_vec(),
            files_removed: Vec::new(),
            db_changes: Vec::new(),
        })
    }

    fn execute_remove(
        &self,
        _db: &Database,
        index: usize,
        _name: &str,
        _version: &str,
        files: &[String],
    ) -> Result<CompletedOp, String> {
        // Backup files before removal
        for file in files {
            self.backup_file(file.as_bytes())?;
        }

        Ok(CompletedOp {
            op_index: index,
            files_created: Vec::new(),
            files_removed: files.to_vec(),
            db_changes: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)] // full upgrade context, forwarded to remove/install
    fn execute_upgrade(
        &self,
        db: &Database,
        index: usize,
        name: &str,
        old_version: &str,
        new_version: &str,
        old_files: &[String],
        new_files: &[String],
    ) -> Result<CompletedOp, String> {
        // First backup old files
        let remove_result = self.execute_remove(db, index, name, old_version, old_files)?;

        // Then track new files
        let install_result = self.execute_install(db, index, name, new_version, new_files)?;

        Ok(CompletedOp {
            op_index: index,
            files_created: install_result.files_created,
            files_removed: remove_result.files_removed,
            db_changes: Vec::new(),
        })
    }

    /// Backup a file before removal
    fn backup_file(&self, path: &[u8]) -> Result<(), String> {
        // Check if file exists
        if io::access(path, libc::F_OK) != 0 {
            return Ok(());
        }

        // Create backup path
        let mut backup_path = self.backup_dir.clone();
        backup_path.extend_from_slice(path);

        // Create parent directories
        if let Some(parent) = get_parent(&backup_path) {
            mkdir_p(&parent);
        }

        // Copy file to backup
        let src_fd = io::open(path, libc::O_RDONLY, 0);
        if src_fd < 0 {
            return Err(String::from("cannot open file for backup"));
        }

        let dst_fd = io::open(
            &backup_path,
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
            0o644,
        );
        if dst_fd < 0 {
            io::close(src_fd);
            return Err(String::from("cannot create backup file"));
        }

        let mut buf = [0u8; 4096];
        loop {
            let n = io::read(src_fd, &mut buf);
            if n <= 0 {
                break;
            }
            io::write_all(dst_fd, &buf[..n as usize]);
        }

        io::close(src_fd);
        io::close(dst_fd);

        Ok(())
    }

    /// Rollback the transaction
    fn rollback(&mut self, db: &Database) {
        io::write_str(2, b"Rolling back transaction...\n");

        // Reverse through completed operations
        for completed in self.completed.iter().rev() {
            // Remove created files
            for file in &completed.files_created {
                io::unlink(file.as_bytes());
            }

            // Restore removed files from backup
            for file in &completed.files_removed {
                let mut backup_path = self.backup_dir.clone();
                backup_path.extend_from_slice(file.as_bytes());

                if io::access(&backup_path, libc::F_OK) == 0 {
                    // Copy back from backup
                    let _ = copy_file(&backup_path, file.as_bytes());
                }
            }

            // Reverse database changes
            for change in completed.db_changes.iter().rev() {
                match change {
                    DbChange::PackageAdded(name) => {
                        db.delete_package(name);
                    }
                    DbChange::PackageRemoved(_name) => {
                        // Would need to restore from backup - complex
                    }
                    DbChange::FilesRegistered(files) => {
                        for file in files {
                            db.unregister_file(file);
                        }
                    }
                    DbChange::FilesUnregistered(_files) => {
                        // Would need package context to re-register
                    }
                }
            }
        }

        self.cleanup_backup();
        self.state = TransactionState::RolledBack;
    }

    /// Clean up backup directory
    fn cleanup_backup(&self) {
        // Remove backup directory recursively
        rm_rf(&self.backup_dir);
    }

    /// Get transaction state
    pub fn state(&self) -> TransactionState {
        self.state
    }
}

/// Create directory and all parents
fn mkdir_p(path: &[u8]) {
    let mut current = Vec::new();

    for &byte in path {
        if byte == b'/' && !current.is_empty() {
            io::mkdir(&current, 0o755);
        }
        current.push(byte);
    }

    if !current.is_empty() {
        io::mkdir(&current, 0o755);
    }
}

/// Get parent directory of a path
fn get_parent(path: &[u8]) -> Option<Vec<u8>> {
    if let Some(pos) = path.iter().rposition(|&b| b == b'/')
        && pos > 0
    {
        return Some(path[..pos].to_vec());
    }
    None
}

/// Copy a file
fn copy_file(src: &[u8], dst: &[u8]) -> Result<(), String> {
    let src_fd = io::open(src, libc::O_RDONLY, 0);
    if src_fd < 0 {
        return Err(String::from("cannot open source"));
    }

    let dst_fd = io::open(dst, libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC, 0o644);
    if dst_fd < 0 {
        io::close(src_fd);
        return Err(String::from("cannot open destination"));
    }

    let mut buf = [0u8; 4096];
    loop {
        let n = io::read(src_fd, &mut buf);
        if n <= 0 {
            break;
        }
        io::write_all(dst_fd, &buf[..n as usize]);
    }

    io::close(src_fd);
    io::close(dst_fd);
    Ok(())
}

/// Remove directory recursively
fn rm_rf(path: &[u8]) {
    // Simple implementation - just try to remove
    // In a full implementation, we'd enumerate and remove contents
    io::rmdir(path);
}

/// Format u64 to decimal string
fn format_u64(mut n: u64, buf: &mut [u8]) -> &[u8] {
    if n == 0 {
        buf[0] = b'0';
        return &buf[..1];
    }

    let mut i = buf.len();
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    &buf[i..]
}
