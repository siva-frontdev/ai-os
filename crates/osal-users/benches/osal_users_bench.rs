use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osal_users::{Credential, DefaultUserManager, UserManager, UserSession, Uid};

fn bench_credential_creation(c: &mut Criterion) {
    c.bench_function("credential_creation_password", |b| {
        b.iter(|| {
            Credential::Password {
                hash: black_box("$argon2id$v=19$m=65536,t=3,p=4$...".into()),
            }
        })
    });

    c.bench_function("credential_creation_key", |b| {
        b.iter(|| {
            Credential::Key {
                public_key: black_box("ssh-ed25519 AAAAC3...".into()),
            }
        })
    });

    c.bench_function("credential_creation_token", |b| {
        b.iter(|| {
            Credential::Token {
                token: black_box("tok_abcdef123456".into()),
            }
        })
    });

    c.bench_function("credential_creation_none", |b| b.iter(|| Credential::None));
}

fn bench_user_session_creation(c: &mut Criterion) {
    c.bench_function("user_session_creation", |b| {
        b.iter(|| {
            UserSession {
                uid: black_box(Uid(1000)),
                username: black_box("bench-user".into()),
                started_at: black_box(chrono::Utc::now()),
                access_token: black_box("tok_bench_session".into()),
            }
        })
    });
}

fn bench_credential_serialize(c: &mut Criterion) {
    let cred = Credential::Password {
        hash: "some_hash_value".into(),
    };
    c.bench_function("credential_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&cred)).unwrap())
    });

    let session = UserSession {
        uid: Uid(1000),
        username: "bench".into(),
        started_at: chrono::Utc::now(),
        access_token: "tok_bench".into(),
    };
    c.bench_function("user_session_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&session)).unwrap())
    });
}

fn bench_default_user_manager_methods(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mgr = DefaultUserManager;
    let ctx = osal_capabilities::CapabilityContext::new("bench");

    c.bench_function("default_user_manager_current_user", |b| {
        b.to_async(&rt)
            .iter(|| mgr.current_user(black_box(&ctx)))
    });

    c.bench_function("default_user_manager_enumerate_users", |b| {
        b.to_async(&rt)
            .iter(|| mgr.enumerate_users(black_box(&ctx)))
    });

    c.bench_function("default_user_manager_enumerate_groups", |b| {
        b.to_async(&rt)
            .iter(|| mgr.enumerate_groups(black_box(&ctx)))
    });
}

criterion_group!(
    benches,
    bench_credential_creation,
    bench_user_session_creation,
    bench_credential_serialize,
    bench_default_user_manager_methods,
);
criterion_main!(benches);
