use std::collections::HashMap;

use handlebars::Handlebars;
use ntex::http::{Client, StatusCode};
use ntex::web::error::InternalError;
use ntex::web::{self, App, HttpRequest, HttpResponse, HttpResponseBuilder, HttpServer};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use rusttype::Font;
use serde_json::{json, Value};
use svg::Document;
use text_svg::Text;

struct AppState {
    token: String,
    font: Font<'static>,
}

async fn index(req: HttpRequest) -> Result<HttpResponse, InternalError<String>> {
    let state: &AppState = req.app_state().unwrap();
    let reg = Handlebars::new();
    let client = Client::default();
    let resp = client
        .get("http://v2.jinrishici.com/sentence")
        .header("X-User-Token", &state.token)
        .send()
        .await
        .map_err(|e| InternalError::default(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?
        .json::<HashMap<String, Value>>()
        .await
        .map_err(|e| InternalError::default(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR))?;
    if let Some(Value::Object(data)) = resp.get("data") {
        if let (Some(Value::String(content)), Some(Value::Object(origin))) =
            (data.get("content"), data.get("origin"))
        {
            if let (
                Some(Value::String(title)),
                Some(Value::String(dynasty)),
                Some(Value::String(author)),
            ) = (
                origin.get("title"),
                origin.get("dynasty"),
                origin.get("author"),
            ) {
                let author = format!("——《{}》{}·{}", title, dynasty, author);
                let content_text = Text::builder()
                    .size(112.0 / 2.0)
                    .build(&state.font, &content);
                let content_document = Document::new()
                    .set("width", content_text.bounding_box.max.x)
                    .set("height", content_text.bounding_box.max.y * 1.5)
                    .add(content_text.path.set("fill", "#cca4e3"))
                    .to_string();
                let author_text = Text::builder()
                    .size(112.0 / 3.0)
                    .build(&state.font, &author);
                let author_document = Document::new()
                    .set("width", author_text.bounding_box.max.x)
                    .set("height", author_text.bounding_box.max.y * 1.5)
                    .add(author_text.path.set("fill", "#e4c6d0"))
                    .to_string();

                let page = include_str!("./page.html");
                return Ok(HttpResponseBuilder::new(StatusCode::OK)
                    .content_type("text/html")
                    .body(
                        reg.render_template(
                            page,
                            &json!({
                                "content": &content_document,
                                "author": &author_document,
                            }),
                        )
                        .map_err(|e| {
                            InternalError::default(e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                        })?,
                    ));
            }
        }
    }
    Ok(HttpResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR).finish())
}

#[ntex::main]
async fn main() -> std::io::Result<()> {
    let token = match std::fs::read_to_string(".token") {
        Ok(token) => token,
        Err(_) => {
            let client = Client::default();
            let resp = client
                .get("https://v2.jinrishici.com/token")
                .send()
                .await
                .unwrap()
                .json::<HashMap<String, Value>>()
                .await
                .unwrap();
            let token = resp.get("data").unwrap().as_str().unwrap();
            std::fs::write(".token", token)?;
            token.to_string()
        }
    };

    let font_data = include_bytes!("font.ttf");
    let font = Font::try_from_bytes(font_data).unwrap();

    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder.set_private_key_file("/etc/letsencrypt/live/example.com/privkey.pem", SslFiletype::PEM).unwrap();
    builder.set_certificate_chain_file("/etc/letsencrypt/live/example.com/fullchain.pem").unwrap();

    HttpServer::new(move || {
        App::new()
            .state(AppState {
                token: token.clone(),
                font: font.clone(),
            })
            .route("/", web::get().to(index))
    })
    .bind_openssl("0.0.0.0:443", builder)?
    .run()
    .await
}
