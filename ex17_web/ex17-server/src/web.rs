use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use log::info;
use rocket::fs::NamedFile;
use rocket::{get, routes, Config};
use rocket_dyn_templates::{context, Template};

const ASSETS_DIR: &str = "ex17-server/public";

pub async fn serve_web(addr: SocketAddr) -> Result<(), rocket::Error> {
    info!("Web admin console listening on {}", &addr);
    let figment = Config::figment();
    let mut config = Config::default();
    config.address = addr.ip();
    config.port = addr.port();
    rocket::build()
        .configure(figment.merge(config))
        .attach(Template::fairing())
        .mount("/", routes![index, assets])
        .launch()
        .await
        .map(|_| ())
}

#[get("/<asset..>")]
async fn assets(asset: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(ASSETS_DIR).join(asset))
        .await
        .ok()
}

#[get("/")]
fn index() -> Template {
    Template::render("index", context! {})
}
