use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::PathBuf;

use osal_core::FileKind;
use osal_filesystem::{DirEntry, FileEvent, FileMetadata, TempFile};

fn bench_file_event_creation(c: &mut Criterion) {
    let ts = Utc::now();

    c.bench_function("FileEvent::Created", |b| {
        b.iter(|| FileEvent::Created {
            path: black_box(PathBuf::from("/tmp/test.txt")),
            timestamp: black_box(ts),
        })
    });

    c.bench_function("FileEvent::Renamed", |b| {
        b.iter(|| FileEvent::Renamed {
            from: black_box(PathBuf::from("/tmp/a")),
            to: black_box(PathBuf::from("/tmp/b")),
            timestamp: black_box(ts),
        })
    });

    c.bench_function("FileEvent::MetadataChanged", |b| {
        b.iter(|| FileEvent::MetadataChanged {
            path: black_box(PathBuf::from("/tmp/test.txt")),
            timestamp: black_box(ts),
        })
    });
}

fn bench_file_event_to_osal(c: &mut Criterion) {
    let ts = Utc::now();
    let created = FileEvent::Created {
        path: PathBuf::from("/tmp/f"),
        timestamp: ts,
    };
    let renamed = FileEvent::Renamed {
        from: PathBuf::from("/a"),
        to: PathBuf::from("/b"),
        timestamp: ts,
    };

    c.bench_function("FileEvent::to_osal (Created)", |b| {
        b.iter(|| black_box(&created).to_osal_event())
    });

    c.bench_function("FileEvent::to_osal (Renamed — None)", |b| {
        b.iter(|| black_box(&renamed).to_osal_event())
    });
}

fn bench_file_metadata_construction(c: &mut Criterion) {
    let ts = Utc::now();

    c.bench_function("FileMetadata::new", |b| {
        b.iter(|| FileMetadata {
            path: black_box(PathBuf::from("/home/user/doc.txt")),
            size: black_box(4096),
            created: black_box(ts),
            modified: black_box(ts),
            accessed: black_box(ts),
            permissions: black_box(0o644),
            file_type: black_box(FileKind::File),
            is_hidden: black_box(false),
        })
    });
}

fn bench_dir_entry_construction(c: &mut Criterion) {
    let ts = Utc::now();

    c.bench_function("DirEntry::new", |b| {
        b.iter(|| DirEntry {
            name: black_box("doc.txt".into()),
            path: black_box(PathBuf::from("/home/user/doc.txt")),
            file_type: black_box(FileKind::File),
            size: black_box(4096),
            modified: black_box(ts),
        })
    });
}

fn bench_temp_file_creation(c: &mut Criterion) {
    c.bench_function("TempFile::new", |b| {
        b.iter(|| TempFile::new(black_box(PathBuf::from("/tmp/bench"))))
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(std::time::Duration::from_millis(200));
    targets = bench_file_event_creation, bench_file_event_to_osal,
              bench_file_metadata_construction, bench_dir_entry_construction,
              bench_temp_file_creation
}

criterion_main!(benches);
