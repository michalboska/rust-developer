use std::ops::Deref;
use std::time::SystemTime;

use lazy_static::lazy_static;
use log::info;
use sqlx::{Acquire, Pool, Row, Sqlite, Transaction};
use thiserror::Error;
use uuid::Uuid;

use ex17_shared::message::Message;

use crate::users::UserError::{AuthenticationFailed, NoSuchUser, Sql, UserAlreadyExists};

pub type UserResult<T> = Result<T, UserError>;
pub type UserResultVoid = UserResult<()>;

pub struct User {
    pub id: String,
    pub name: String,
}

#[derive(sqlx::FromRow)]
struct DbUser {
    id: String,
    name: String,
    active: u8,
    password: String,
    salt: String,
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error("User with name {0} not found")]
    NoSuchUser(String),
    #[error("User with name {0} already exists")]
    UserAlreadyExists(String),
    #[error("Authentication failed")]
    AuthenticationFailed,
}

pub struct UserService {
    pool: Pool<Sqlite>,
}

impl UserService {
    pub async fn new(pool: Pool<Sqlite>) -> Result<UserService, UserError> {
        let inst = UserService { pool };
        inst.ensure_schema_exists().await?;
        Ok(inst)
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> UserResult<User> {
        let mut tx = self.pool.begin().await?;
        match UserService::get_user_by_name(&mut tx, username).await? {
            None => Err(AuthenticationFailed),
            Some(db_user) => {
                let expected_digest = UserService::get_passwd_digest(password, &db_user.salt);
                if db_user.active == 1 && db_user.password == expected_digest {
                    Ok(User {
                        id: db_user.id,
                        name: db_user.name,
                    })
                } else {
                    Err(AuthenticationFailed)
                }
            }
        }
    }

    pub async fn signup(&self, username: &str, password: &str) -> UserResult<User> {
        let mut tx = self.pool.begin().await?;
        match UserService::get_user_by_name(&mut tx, username).await? {
            Some(_) => Err(UserAlreadyExists(username.to_string())),
            None => {
                let new_id = Uuid::new_v4().to_string();
                let salt = Uuid::new_v4().to_string();
                let passwd_digest = UserService::get_passwd_digest(password, &salt);
                sqlx::query(
                    "insert into users(id, name, active, salt, password) values(?,?,?,?,?)",
                )
                .bind(&new_id)
                .bind(username)
                .bind(1)
                .bind(salt)
                .bind(passwd_digest)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok(User {
                    id: new_id,
                    name: username.to_string(),
                })
            }
        }
    }

    pub async fn change_password(&self, user: &User, new_password: &str) -> UserResultVoid {
        let mut tx = self.pool.begin().await?;
        let new_salt = Uuid::new_v4().to_string();
        let passwd_digest = UserService::get_passwd_digest(new_password, &new_salt);
        let result = sqlx::query("update users set password=?, salt=? where id=?")
            .bind(passwd_digest)
            .bind(new_salt)
            .bind(&user.id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 1 {
            tx.commit().await?;
            Ok(())
        } else {
            Err(NoSuchUser(user.name.clone()))
        }
    }

    pub async fn save_user_message(&self, user: &User, message: &Message) -> UserResultVoid {
        let mut tx = self.pool.begin().await?;
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let message_id = Uuid::new_v4().to_string();
        let message_str = match message {
            Message::File(filename, _) => Some(format!("[Shared file {}]", filename)),
            Message::Image(_) => Some("[Shared an image]".to_string()),
            Message::Text(text) => Some(text.clone()),
            _ => None,
        };
        if let Some(message) = message_str {
            sqlx::query("insert into user_messages(id, author_id, message, sent_at_instant) values(?,?,?,?)")
                .bind(&message_id)
                .bind(&user.id)
                .bind(message)
                .bind(timestamp as i64)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        Ok(())
    }

    async fn get_user_by_name(
        tx: &mut Transaction<'_, Sqlite>,
        name: &str,
    ) -> UserResult<Option<DbUser>> {
        sqlx::query_as("select id,name,active,password,salt from users where name=?")
            .bind(name)
            .fetch_optional(&mut **tx)
            .await
            .map_err(UserError::from)
    }

    fn get_passwd_digest(passwd: &str, salt: &str) -> String {
        let passwd_with_salt = format!("{}{}", passwd, salt);
        sha256::digest(passwd_with_salt)
    }

    async fn ensure_schema_exists(&self) -> Result<(), UserError> {
        let mut connection = self.pool.acquire().await?;
        let mut tx = connection.begin().await?;
        let result = sqlx::query("select name from sqlite_master where type = 'table'")
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .fold(0, |acc, elem| {
                let tbl_name = elem.get::<String, usize>(0);
                match tbl_name.as_str() {
                    "user_messages" | "users" => acc + 1,
                    _ => acc,
                }
            });
        if result == 2 {
            tx.commit().await?;
            return Ok(());
        }
        info!("Creating a new database as it did not exist before.");
        for sql in INIT_SQL.deref() {
            let result = sqlx::query(sql)
                .execute(&mut *tx)
                .await
                .map(|_| ())
                .map_err(Sql);
            if result.is_err() {
                return result;
            }
        }
        tx.commit().await?;
        Ok(())
    }
}

lazy_static! {
    static ref INIT_SQL: Vec<&'static str> = vec![
        r##"
        create table main.users (
        id       TEXT            not null
            constraint users_pk
                primary key,
        name     TEXT,
        active   INTEGER,
        password TEXT not null,
        salt     TEXT not null
    );
    "##,
        "create unique index uq_users_name ON users (name);",
        r##"
        create table main.user_messages (
        id              TEXT    not null
            constraint user_messages_pk
                primary key,
        author_id       TEXT    not null,
        message         TEXT    not null,
        sent_at_instant INTEGER not null,
        foreign key (author_id) REFERENCES users (id)
    );
    "##,
        "create index idx_user_messages_author_id on user_messages (author_id);",
    ];
}

impl From<DbUser> for User {
    fn from(value: DbUser) -> Self {
        User {
            id: value.id,
            name: value.name,
        }
    }
}
