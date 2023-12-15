use std::fmt::Debug;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use log::{error, info};
use rocket::form::Form;
use rocket::fs::NamedFile;
use rocket::http::{CookieJar, Status};
use rocket::response::Redirect;
use rocket::{get, post, routes, Config};
use rocket_dyn_templates::{context, Template};
use thiserror::Error;

use crate::users::{UserError, UserService};
use crate::web_user::{LoggedUser, LoginForm, RegisterUserForm, UpdateUserForm};

const ASSETS_DIR: &str = "ex18-server/public";

pub async fn serve_web(addr: SocketAddr) -> Result<(), rocket::Error> {
    info!("Web admin console listening on {}", &addr);

    let figment = Config::figment();
    let mut config = Config::default();
    config.address = addr.ip();
    config.port = addr.port();
    rocket::build()
        .configure(figment.merge(config))
        .attach(Template::fairing())
        .mount(
            "/",
            routes![
                index,
                index_redirect,
                login,
                login_execute,
                login_redirect,
                signup,
                update_user,
                assets
            ],
        )
        .launch()
        .await
        .map(|_| ())
}

#[get("/", rank = 1)]
async fn index(_u: LoggedUser) -> Result<Template, Status> {
    let user_service = UserService::instance();
    let all_users = user_service.get_all_users().await?;
    let all_messages = user_service.get_user_messages().await?;
    Ok(Template::render(
        "index",
        context! {users: all_users, messages: all_messages},
    ))
}

#[get("/", rank = 2)]
fn index_redirect() -> Redirect {
    Redirect::to("/login")
}

#[get("/login", rank = 1)]
fn login_redirect(_u: LoggedUser) -> Redirect {
    Redirect::to("/")
}

#[get("/login", rank = 2)]
fn login() -> Template {
    Template::render("login", context! {})
}

#[post("/login", data = "<login_form>")]
async fn login_execute(
    login_form: Form<LoginForm>,
    cookies: &CookieJar<'_>,
) -> Result<Redirect, Template> {
    let failed_login = || Template::render("login", context! {failed:true});
    let user = UserService::instance()
        .authenticate(&login_form.login, &login_form.password)
        .await
        .map_err(|_| failed_login())?;
    if user.is_admin {
        LoggedUser::set_login_cookie(cookies, user.id);
        Ok(Redirect::to("/"))
    } else {
        Err(failed_login())
    }
}

#[post("/update-user", data = "<update_user_form>")]
async fn update_user(
    _user: LoggedUser,
    update_user_form: Form<UpdateUserForm>,
) -> Result<Redirect, Status> {
    UserService::instance()
        .update_user(
            &update_user_form.user_id,
            update_user_form.is_admin,
            update_user_form.is_active,
        )
        .await
        .map_err(|_| Status::InternalServerError)
        .map(|_| Redirect::to("/"))
}

#[post("/signup", data = "<signup_form>")]
async fn signup(
    _user: LoggedUser,
    signup_form: Form<RegisterUserForm>,
) -> Result<Redirect, Status> {
    UserService::instance()
        .signup(&signup_form.login, &signup_form.password)
        .await
        .map_err(|_| Status::InternalServerError)
        .map(|_| Redirect::to("/"))
}

#[get("/static/<asset..>")]
async fn assets(asset: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(ASSETS_DIR).join(asset))
        .await
        .ok()
}

#[derive(Debug, Error)]
enum WebError {
    #[error(transparent)]
    User(#[from] UserError),
}

impl From<UserError> for Status {
    fn from(_: UserError) -> Self {
        Status::InternalServerError
    }
}
