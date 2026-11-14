//! Compare a bounded block-import workload against the original crate.
use std::time::Instant;

const BLOCKS: u64 = 512;
const ENTRIES_PER_BLOCK: u64 = 256;
const VALUE_BYTES: usize = 1024;

fn key(id: u64) -> [u8; 32] {
	let mut key = [0; 32];
	// Scatter sequential IDs across the hash index without another dependency.
	for (n, chunk) in key.chunks_exact_mut(8).enumerate() {
		chunk.copy_from_slice(
			&id.wrapping_mul(0x9e3779b97f4a7c15).rotate_left(n as u32 * 13).to_le_bytes(),
		);
	}
	key
}

macro_rules! run {
    ($backend:ident) => {{
        let dir = tempfile::tempdir().unwrap();
        let options = $backend::Options::with_columns(dir.path(), 1);
        let db = $backend::Db::open_or_create(&options).unwrap();
        let started = Instant::now();
        for block in 0..BLOCKS {
            let value = vec![(block % 251) as u8; VALUE_BYTES];
            db.commit((0..ENTRIES_PER_BLOCK).map(|entry| {
                (0, key(block * ENTRIES_PER_BLOCK + entry), Some(value.clone()))
            })).unwrap();
            // Read state from earlier blocks as well as the current in-memory overlay.
            for lookback in 0..16 {
                let prior_block = block.saturating_sub(lookback * 7);
                assert_eq!(
                    db.get(0, &key(prior_block * ENTRIES_PER_BLOCK)).unwrap(),
                    Some(vec![(prior_block % 251) as u8; VALUE_BYTES]),
                );
            }
        }
        let submitted = started.elapsed();
        drop(db); // Include draining queued commits and durable writes in the total.
        let durable = started.elapsed();
        let db = $backend::Db::open(&options).unwrap();
        for block in 0..BLOCKS {
            assert_eq!(db.get(0, &key(block * ENTRIES_PER_BLOCK)).unwrap(),
                Some(vec![(block % 251) as u8; VALUE_BYTES]));
        }
        println!("{} blocks={} entries_per_block={} value_bytes={} submit_ms={} durable_ms={} durable_ms_per_block={:.3}",
            stringify!($backend), BLOCKS, ENTRIES_PER_BLOCK, VALUE_BYTES,
            submitted.as_millis(), durable.as_millis(), durable.as_secs_f64() * 1000.0 / BLOCKS as f64);
    }};
}

fn main() {
	match std::env::args().nth(1).as_deref() {
		Some("original") => run!(original),
		Some("patched") => run!(patched),
		_ => panic!("expected original or patched"),
	}
}
