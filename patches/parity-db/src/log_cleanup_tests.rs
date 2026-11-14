// Copyright 2026 Duniter contributors.
// This file is dual-licensed as Apache-2.0 or MIT.

use super::{Db, Options, KEEP_LOGS, MAX_LOG_FILES};
use std::{
	sync::{mpsc, Arc},
	thread,
	time::{Duration, Instant},
};
use tempfile::{tempdir, TempDir};

const TIMEOUT: Duration = Duration::from_secs(5);

// Reproduce an enactment backlog without relying on disk speed or thread scheduling.
fn cleanup_backlog(sync_data: bool) -> (TempDir, Db) {
	let dir = tempdir().unwrap();
	let mut options = Options::with_columns(dir.path(), 1);
	options.with_background_thread = false;
	options.sync_data = sync_data;
	let db = Db::open_or_create(&options).unwrap();
	let limit = if sync_data { MAX_LOG_FILES } else { KEEP_LOGS };
	for n in 0..=limit {
		db.commit([(0, (n as u64).to_le_bytes(), Some(vec![n as u8]))]).unwrap();
		assert!(db.inner.process_commits().unwrap());
		assert!(db.inner.flush_logs(0).unwrap());
		// Recovery bypasses cleanup throttling, allowing the test to stage the backlog.
		while db.inner.enact_logs(true).unwrap() {}
	}
	assert_eq!(db.inner.log.num_dirty_logs(), limit + 1);
	db.commit([(0, b"pending".as_slice(), Some(b"value".to_vec()))]).unwrap();
	assert!(db.inner.process_commits().unwrap());
	assert!(db.inner.flush_logs(0).unwrap());
	(dir, db)
}

fn cleanup_is_signalled(sync_data: bool) {
	let (_dir, db) = cleanup_backlog(sync_data);
	let inner = Arc::clone(&db.inner);
	let worker = thread::spawn(move || inner.enact_logs(false).unwrap());
	let wait = &db.inner.cleanup_worker_wait;
	let signalled = {
		let deadline = Instant::now() + TIMEOUT;
		let mut work = wait.work.lock();
		while !*work {
			let remaining = deadline.saturating_duration_since(Instant::now());
			if remaining.is_zero() {
				break
			}
			wait.cv.wait_for(&mut work, remaining);
		}
		*work
	};
	// Also release the unpatched worker on failure so the test reports instead of hanging.
	db.inner.clean_logs().unwrap();
	assert!(worker.join().unwrap());
	assert!(signalled, "enactment must wake cleanup before waiting for the log cap");
	assert_eq!(db.get(0, b"pending").unwrap(), Some(b"value".to_vec()));
}

fn shutdown_skips_cleanup_wait(sync_data: bool) {
	let (_dir, db) = cleanup_backlog(sync_data);
	db.inner.shutdown();
	let inner = Arc::clone(&db.inner);
	let (tx, rx) = mpsc::channel();
	let worker = thread::spawn(move || tx.send(inner.enact_logs(false).unwrap()).unwrap());
	let result = rx.recv_timeout(TIMEOUT);
	// Rescue the unpatched implementation before asserting the timeout failure.
	db.inner.clean_logs().unwrap();
	worker.join().unwrap();
	assert!(result.expect("shutdown must skip cleanup throttling"));
}

#[test]
fn sync_cleanup_backlog_wakes_worker() {
	cleanup_is_signalled(true);
}

#[test]
fn async_cleanup_backlog_wakes_worker() {
	cleanup_is_signalled(false);
}

#[test]
fn sync_shutdown_skips_cleanup_backlog() {
	shutdown_skips_cleanup_wait(true);
}

#[test]
fn async_shutdown_skips_cleanup_backlog() {
	shutdown_skips_cleanup_wait(false);
}
