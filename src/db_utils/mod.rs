use crate::diesel_migrations::MigrationHarness;
use diesel::{
    connection::Connection,
    r2d2::{ConnectionManager, Pool, PooledConnection},
    PgConnection,
};
use diesel_migrations::EmbeddedMigrations;
use r2d2_redis::{r2d2, RedisConnectionManager};
use std::time::Duration;

const CACHE_POOL_MAX_OPEN: u32 = 16;
const CACHE_POOL_MIN_IDLE: u32 = 8;
const CACHE_POOL_TIMEOUT_SECONDS: u64 = 1;
const CACHE_POOL_EXPIRE_SECONDS: u64 = 60;

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

pub fn get_db_pool(db_url: &str) -> Pool<ConnectionManager<PgConnection>> {
    let manager = ConnectionManager::<PgConnection>::new(db_url);
    Pool::builder()
        .build(manager)
        .expect("Error building a db connection pool")
}

pub fn get_db_connection() -> PooledConnection<ConnectionManager<PgConnection>> {
    let db_url = dotenv::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = get_db_pool(&db_url);
    pool.get().expect("couldn't get db connection from pool")
}

pub fn get_redis_pool(redis_url: &str) -> Pool<RedisConnectionManager> {
    let manager =
        RedisConnectionManager::new(redis_url).expect("Error with redis connection manager");
    Pool::builder()
        .max_size(CACHE_POOL_MAX_OPEN)
        .max_lifetime(Some(Duration::from_secs(CACHE_POOL_EXPIRE_SECONDS)))
        .min_idle(Some(CACHE_POOL_MIN_IDLE))
        .build(manager)
        .expect("Error building a redis connection pool")
}

pub fn get_redis_connection(
    pool: &r2d2::Pool<RedisConnectionManager>,
) -> PooledConnection<RedisConnectionManager> {
    pool.get_timeout(Duration::from_secs(CACHE_POOL_TIMEOUT_SECONDS))
        .expect("couldn't get db connection from pool")
}
