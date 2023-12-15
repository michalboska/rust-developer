use std::ops::Deref;
use std::sync::OnceLock;
use std::time::SystemTime;

use lazy_static::lazy_static;
use log::info;
use rocket::tokio::runtime::Handle;
use serde_derive::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Acquire, Pool, Row, Sqlite, Transaction};
use thiserror::Error;
use uuid::Uuid;

use ex18_shared::message::Message;

use crate::users::UserError::{AuthenticationFailed, NoSuchUser, Sql, UserAlreadyExists};

pub type UserResult<T> = Result<T, UserError>;
pub type UserResultVoid = UserResult<()>;

const SQLITE_DB_FILE: &str = "server.db";
static INSTANCE: OnceLock<UserService> = OnceLock::new();

#[derive(Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub is_admin: bool,
}

#[derive(sqlx::FromRow)]
struct DbUser {
    id: String,
    name: String,
    active: u8,
    admin: u8,
    password: String,
    salt: String,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct UserMessageView {
    pub author_name: String,
    pub message: String,
    pub sent_at_instant: i64,
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
    pub fn instance() -> &'static UserService {
        INSTANCE
            .get_or_init(|| Handle::current().block_on(async { UserService::new().await.unwrap() }))
    }

    pub async fn get_all_users(&self) -> UserResult<Vec<User>> {
        Ok(sqlx::query_as::<Sqlite, DbUser>("select * from users")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(User::from)
            .collect::<Vec<User>>())
    }

    pub async fn get_user_by_id(&self, id: &str) -> UserResult<User> {
        sqlx::query_as::<Sqlite, DbUser>(
            "select id,name,active,admin,password,salt from users where id=?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .map(User::from)
        .ok_or(NoSuchUser(id.to_string()))
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> UserResult<User> {
        let mut tx = self.pool.begin().await?;
        match UserService::get_user_by_name(&mut tx, username).await? {
            None => Err(AuthenticationFailed),
            Some(db_user) => {
                let expected_digest = UserService::get_passwd_digest(password, &db_user.salt);
                if db_user.active == 1 && db_user.password == expected_digest {
                    Ok(User::from(db_user))
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
                    is_active: true,
                    is_admin: false,
                })
            }
        }
    }

    pub async fn update_user(
        &self,
        user_id: &str,
        is_admin: bool,
        is_active: bool,
    ) -> UserResultVoid {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query("update users set active=?, admin=? where id=?")
            .bind(if is_active { 1 } else { 0 })
            .bind(if is_admin { 1 } else { 0 })
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() > 0 {
            tx.commit().await?;
            Ok(())
        } else {
            Err(NoSuchUser(user_id.to_string()))
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

    pub async fn get_user_messages(&self) -> UserResult<Vec<UserMessageView>> {
        Ok(sqlx::query_as::<Sqlite, UserMessageView>(
            r#"
        select u.name as author_name, m.message, m.sent_at_instant
        from user_messages m
                 join main.users u on u.id = m.author_id
        order by m.sent_at_instant desc"#,
        )
        .fetch_all(&self.pool)
        .await?)
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
        sqlx::query_as("select id,name,active,admin,password,salt from users where name=?")
            .bind(name)
            .fetch_optional(&mut **tx)
            .await
            .map_err(UserError::from)
    }

    fn get_passwd_digest(passwd: &str, salt: &str) -> String {
        let passwd_with_salt = format!("{}{}", passwd, salt);
        sha256::digest(passwd_with_salt)
    }

    async fn new() -> Result<UserService, UserError> {
        let connect_options = SqliteConnectOptions::new()
            .filename(SQLITE_DB_FILE)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(connect_options)
            .await?;
        let inst = UserService { pool };
        inst.ensure_schema_exists().await?;
        Ok(inst)
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
        let admin_user = self.signup("admin", "admin").await?;
        self.update_user(&admin_user.id, true, true).await?;
        info!("Created first admin user: admin/admin don't forget to change the credentials.");
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
        admin    INTEGER default 0,
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
        "create index idx_user_messages_sent_at on user_messages (sent_at_instant desc);",
    ];
}

impl From<DbUser> for User {
    fn from(value: DbUser) -> Self {
        User {
            id: value.id,
            name: value.name,
            is_active: value.active > 0,
            is_admin: value.admin > 0,
        }
    }
}
