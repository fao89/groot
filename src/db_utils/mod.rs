use diesel::{
    connection::Connection,
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use std::time::Duration;

pub fn run_migrations(db_url: &str) {
    embed_migrations!();
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
    embedded_migrations::run_with_output(&connection.unwrap(), &mut std::io::stdout())
        .expect("Error running migrations");
}

pub fn get_pool(db_url: &str) -> Pool<ConnectionManager<PgConnection>> {
    let manager = ConnectionManager::<PgConnection>::new(db_url);
    Pool::builder()
        .build(manager)
        .expect("Error building a connection pool")
}
