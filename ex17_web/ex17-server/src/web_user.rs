use rocket::http::{Cookie, CookieJar, Status};
use rocket::request::{FromRequest, Outcome};
use rocket::{FromForm, Request};

use crate::users::{User, UserError, UserService};

pub struct LoggedUser(pub User);

const COOKIE_USER_ID: &str = "user_id";

#[rocket::async_trait]
impl<'r> FromRequest<'r> for LoggedUser {
    type Error = UserError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let cookie = match request.cookies().get_private(COOKIE_USER_ID) {
            None => return Outcome::Forward(Status::Unauthorized),
            Some(c) => c,
        };
        match UserService::instance().get_user_by_id(cookie.value()).await {
            Ok(user) if user.is_admin => Outcome::Success(LoggedUser(user)),
            Ok(_) => {
                request.cookies().remove_private(COOKIE_USER_ID);
                Outcome::Forward(Status::Unauthorized)
            }
            Err(UserError::NoSuchUser(_)) => Outcome::Forward(Status::Unauthorized),
            Err(e) => Outcome::Error((Status::InternalServerError, e)),
        }
    }
}

impl LoggedUser {
    pub fn set_login_cookie(cookie_jar: &CookieJar<'_>, user_id: String) {
        cookie_jar.add_private(Cookie::new(COOKIE_USER_ID, user_id));
    }
}

#[derive(FromForm)]
pub struct LoginForm {
    pub login: String,
    pub password: String,
}

#[derive(FromForm)]
pub struct UpdateUserForm {
    pub user_id: String,
    pub is_active: bool,
    pub is_admin: bool,
}

#[derive(FromForm)]
pub struct RegisterUserForm {
    pub login: String,
    pub password: String,
}
