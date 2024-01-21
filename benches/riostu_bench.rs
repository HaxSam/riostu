use std::io::BufReader;

use criterion::{criterion_group, criterion_main, Criterion};
use riostu::RemoteIO;
use zip::ZipArchive;

static TEST_URL: &str =
    "https://oxygenos.oneplus.net/OnePlus8TOxygen_15.E.29_OTA_0290_all_2110091931_downgrade";

fn create_rio(c: &mut Criterion) {
    c.bench_function("Creation of a RemoteIO", |b| {
        b.iter(|| RemoteIO::new(TEST_URL).unwrap())
    });
}

fn create_zip(c: &mut Criterion) {
    c.bench_function("Creation of a Zip", |b| {
        b.iter(|| {
            let rio = RemoteIO::new(TEST_URL).unwrap().wait();
            let buf_reader = BufReader::new(rio);

            ZipArchive::new(buf_reader).unwrap()
        })
    });
}

fn read_files_from_zip(c: &mut Criterion) {
    c.bench_function("Reading content out of a Zip", |b| {
        b.iter(|| {
            let rio = RemoteIO::new(TEST_URL).unwrap().wait();
            let buf_reader = BufReader::new(rio);

            let mut zip = ZipArchive::new(buf_reader).unwrap();
            for i in 0..zip.len() {
                let _ = zip.by_index(i).unwrap();
            }
        })
    });
}

criterion_group!(benches, create_rio, create_zip, read_files_from_zip);
criterion_main!(benches);
