// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! The reader pool: a few read-only connections shared by the process's
//! threads. In WAL mode readers never wait for writers; each read runs in
//! one transaction, so it sees one consistent snapshot. Reads must stay
//! short, because an open read transaction keeps the WAL from being reset.

use std::sync::{Condvar, Mutex, PoisonError};

use rusqlite::{Connection, Transaction};

use crate::error::StoreError;

#[derive(Debug)]
pub(crate) struct ReaderPool {
    idle: Mutex<Vec<Connection>>,
    returned: Condvar,
    size: usize,
}

/// A connection taken from the pool; goes back when dropped, also when the
/// read panics.
struct Lease<'a> {
    pool: &'a ReaderPool,
    conn: Option<Connection>,
}

impl Drop for Lease<'_> {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool
                .idle
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(conn);
            self.pool.returned.notify_one();
        }
    }
}

impl ReaderPool {
    pub(crate) fn new(connections: Vec<Connection>) -> Self {
        let size = connections.len();
        Self {
            idle: Mutex::new(connections),
            returned: Condvar::new(),
            size,
        }
    }

    pub(crate) fn size(&self) -> usize {
        self.size
    }

    fn lease(&self) -> Lease<'_> {
        let mut idle = self.idle.lock().unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(conn) = idle.pop() {
                return Lease {
                    pool: self,
                    conn: Some(conn),
                };
            }
            idle = self
                .returned
                .wait(idle)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    pub(crate) fn read<R, E>(
        &self,
        f: impl FnOnce(&Transaction<'_>) -> Result<R, E>,
    ) -> Result<R, E>
    where
        E: From<StoreError>,
    {
        let mut lease = self.lease();
        let conn = lease.conn.as_mut().ok_or(StoreError::Closed)?;
        let tx = conn.transaction().map_err(StoreError::from)?;
        let result = f(&tx);
        // Ending the read transaction; nothing was written.
        drop(tx);
        result
    }
}
