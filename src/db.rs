use chrono::prelude::*;
use serde::Serialize;
use sqlx::{FromRow, Row, SqlitePool};

pub const DB_URL: &str = "afisha.db";

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub tg_id: u64,
    pub city: String,
    pub tags: Vec<String>,
    pub notification_time: NaiveTime,
    pub events_interval: u32,
}

impl User {
    pub fn default() -> Self {
        Self {
            id: -1,
            tg_id: 1,
            city: "w".into(),
            tags: Vec::new(),
            notification_time: Local::now().time(),
            events_interval: 1,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct UserFilter {
    pub id: Option<i64>,
    pub tg_id: Option<u64>,
    pub city: Option<String>,
    pub tags: Option<Vec<String>>,
    pub notification_time: Option<NaiveTime>,
    pub events_interval: Option<u32>,
}

pub async fn init_db(pool: &SqlitePool) {
    let mut tx = pool.begin().await.unwrap();

    let result = sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS users (
            id integer primary key,
            tg_id text,
            city text,
            tags text,
            notification_time text,
            events_interval integer
        )
        ",
    )
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();
}

pub async fn insert_user(pool: &SqlitePool, user: User) {
    let mut tx = pool.begin().await.unwrap();

    let result = sqlx::query(
        "
        INSERT INTO users (tg_id, city, tags, notification_time, events_interval)
        VALUES ($1, $2, $3, $4, $5)
        ",
    )
    .bind(serde_json::to_string(&user.tg_id).unwrap())
    .bind(user.city)
    .bind(serde_json::to_string(&user.tags).unwrap())
    .bind(user.notification_time)
    .bind(user.events_interval)
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();
}

pub async fn get_user(pool: &SqlitePool, id: u64) -> Option<User> {
    let mut tx = pool.begin().await.unwrap();

    let rows = sqlx::query(&format!(
        "
        SELECT * FROM users
        WHERE tg_id = {id}
        "
    ))
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    let mut users = Vec::new();

    if rows.last().is_none() {
        return None;
    }

    for row in rows {
        users.push(User {
            id: row.get(0),
            tg_id: serde_json::from_str(row.get(1)).unwrap(),
            city: row.get(2),
            tags: serde_json::from_str(row.get(3)).unwrap(),
            notification_time: row.get(4),
            events_interval: row.get(5),
        });
    }

    tx.commit().await.unwrap();

    Some(users[0].clone())
}

pub async fn get_all_users(pool: &SqlitePool) -> Option<Vec<User>> {
    let mut tx = pool.begin().await.unwrap();

    let rows = sqlx::query(&format!(
        "
        SELECT * FROM users
        "
    ))
    .fetch_all(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    let mut users = Vec::new();

    if rows.last().is_none() {
        return None;
    }

    for row in rows {
        users.push(User {
            id: row.get(0),
            tg_id: serde_json::from_str(row.get(1)).unwrap(),
            city: row.get(2),
            tags: serde_json::from_str(row.get(3)).unwrap(),
            notification_time: row.get(4),
            events_interval: row.get(5),
        });
    }

    Some(users)
}

pub async fn update_user(
    pool: &SqlitePool,
    values: UserFilter,
    old_user: User,
) -> Result<(), sqlx::Error> {
    let mut conn = pool.acquire().await?;

    let id_insert = old_user.id;
    let tg_id_insert: i64 = match values.tg_id {
        Some(tg_id) => tg_id.try_into().unwrap(),
        None => old_user.tg_id.try_into().unwrap(),
    };
    let city_insert = match values.city {
        Some(city) => city,
        None => old_user.city,
    };
    let tags_insert = match values.tags {
        Some(tags) => tags,
        None => old_user.tags,
    };
    let notification_time_insert = match values.notification_time {
        Some(notification_time) => notification_time,
        None => old_user.notification_time,
    };
    let events_interval_insert = match values.events_interval {
        Some(events_interval) => events_interval,
        None => old_user.events_interval,
    };

    let result = sqlx::query(
        "
        UPDATE profiles SET id = $1, tg_id = $2, city = $3, tags = $4, notification_time = $5, events_interval = $6
        WHERE id = $1
        "
    )
    .bind(id_insert)
    .bind(tg_id_insert)
    .bind(serde_json::to_string(&tags_insert).unwrap())
    .bind(notification_time_insert)
    .bind(events_interval_insert)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
