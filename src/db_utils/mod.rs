use crate::diesel_migrations::MigrationHarness;
use diesel::{connection::Connection, PgConnection};
use diesel_migrations::EmbeddedMigrations;
use std::time::Duration;

pub fn run_migrations(db_url: &str) {
    pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
    let mut connection = PgConnection::establish(db_url);
    for _ in 0..5 {
        if connection.is_err() {
            println!("Error connecting to database - Retrying...");
            std::thread::sleep(Duration::from_secs(30));
            connection = PgConnection::establish(db_url);
        } else {
            break;
        }
    }
    let _ = &mut connection
        .unwrap()
        .run_pending_migrations(MIGRATIONS)
        .expect("Error running migrations");
}
