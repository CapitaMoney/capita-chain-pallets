#[cfg(test)]
mod tests {
	#[test]
	fn original_and_backport_can_read_each_others_writes() {
		let dir = tempfile::tempdir().unwrap();
		let old_options = original::Options::with_columns(dir.path(), 1);
		let patched_options = patched::Options::with_columns(dir.path(), 1);
		let key = b"key";
		// A multipart value also exercises the large-value layout that changed in 0.5.
		let old_value = vec![17; 100_000];
		let new_value = vec![29; 120_000];
		{
			let db = original::Db::open_or_create(&old_options).unwrap();
			db.commit([(0, key, Some(old_value.clone()))]).unwrap();
		}
		let metadata = std::fs::read(dir.path().join("metadata")).unwrap();
		{
			let db = patched::Db::open(&patched_options).unwrap();
			assert_eq!(db.get(0, key).unwrap(), Some(old_value));
			db.commit([(0, key, Some(new_value.clone()))]).unwrap();
		}
		assert_eq!(std::fs::read(dir.path().join("metadata")).unwrap(), metadata);
		{
			let db = original::Db::open(&old_options).unwrap();
			assert_eq!(db.get(0, key).unwrap(), Some(new_value));
			db.commit([(0, key, None)]).unwrap();
		}
		let db = patched::Db::open(&patched_options).unwrap();
		assert_eq!(db.get(0, key).unwrap(), None);
	}
}
