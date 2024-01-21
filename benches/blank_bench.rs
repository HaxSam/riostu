use criterion::{criterion_group, criterion_main, Criterion};
use riostu::Blank;
use zip::ZipArchive;

fn create_blank(c: &mut Criterion) {
    static T: &str =
        "https://oxygenos.oneplus.net/OnePlus8TOxygen_15.E.29_OTA_0290_all_2110091931_downgrade";

    c.bench_function("create blank", |b| b.iter(|| Blank::new(T).unwrap()));
}

fn create_zip(c: &mut Criterion) {
    static T: &str =
        "https://oxygenos.oneplus.net/OnePlus8TOxygen_15.E.29_OTA_0290_all_2110091931_downgrade";

    c.bench_function("create zip", |b| {
        b.iter(|| {
            let blank = Blank::new(T).unwrap();
            let blank = Blank::make_block(blank);
            let u = std::io::BufReader::new(blank);

            ZipArchive::new(u).unwrap()
        })
    });
}

fn read_files_from_zip(c: &mut Criterion) {
    static T: &str =
        "https://oxygenos.oneplus.net/OnePlus8TOxygen_15.E.29_OTA_0290_all_2110091931_downgrade";

    c.bench_function("read files from zip", |b| {
        b.iter(|| {
            let blank = Blank::new(T).unwrap();
            let blank = Blank::make_block(blank);
            let u = std::io::BufReader::new(blank);

            let mut zip = ZipArchive::new(u).unwrap();
            for i in 0..zip.len() {
                let _ = zip.by_index(i).unwrap();
            }
        })
    });
}

criterion_group!(benches, create_blank, create_zip, read_files_from_zip);
criterion_main!(benches);
