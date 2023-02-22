use std::collections::HashMap;
use std::{error::Error, fs, io};

use actix_web::http::StatusCode;
use actix_web::web::Data;
use actix_web::{get, App, HttpResponse, HttpResponseBuilder, HttpServer};
use handlebars::Handlebars;
use reqwest::header::{HeaderMap, HeaderValue};
use rusttype::Font;
use serde::Deserialize;
use serde_json::{json, Value};
use svg::Document;
use text_svg::Text;

struct AppState {
    token: String,
    font: Font<'static>,
}

#[get("/")]
async fn index(data: Data<AppState>) -> Result<HttpResponse, Box<dyn Error>> {
    let reg = Handlebars::new();
    let mut header = HeaderMap::new();
    let font = &data.font;
    header.append("X-User-Token", HeaderValue::from_str(&data.token)?);
    let client = reqwest::ClientBuilder::new()
        .default_headers(header)
        .build()?;
    let resp = client
        .get("https://v2.jinrishici.com/sentence")
        .send()
        .await?
        .json::<HashMap<String, Value>>()
        .await?;

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
                let content_text = Text::builder().size(112.0 / 2.0).build(font, &content);
                let content_document = Document::new()
                    .set("width", content_text.bounding_box.max.x)
                    .set("height", content_text.bounding_box.max.y * 1.5)
                    .add(content_text.path.set("fill", "#cca4e3"))
                    .to_string();
                let author_text = Text::builder().size(112.0 / 3.0).build(font, &author);
                let author_document = Document::new()
                    .set("width", author_text.bounding_box.max.x)
                    .set("height", author_text.bounding_box.max.y * 1.5)
                    .add(author_text.path.set("fill", "#e4c6d0"))
                    .to_string();

                let page = include_str!("./page.html");
                return Ok(HttpResponseBuilder::new(StatusCode::from_u16(200).unwrap())
                    .content_type("")
                    .body(reg.render_template(
                        page,
                        &json!({
                            "content": &content_document,
                            "author": &author_document,
                        }),
                    )?));
            }
        }
    }
    Ok(HttpResponseBuilder::new(StatusCode::from_u16(500).unwrap()).finish())
}

#[derive(Deserialize)]
struct Token {
    #[allow(dead_code)]
    status: String,
    data: String,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let token = match fs::read_to_string(".token") {
        Ok(token) => token,
        Err(_) => {
            let resp = reqwest::get("https://v2.jinrishici.com/token")
                .await
                .unwrap()
                .json::<Token>()
                .await
                .unwrap();
            fs::write(".token", &resp.data)?;
            resp.data
        }
    };

    let font_data = include_bytes!("font.ttf");
    let font = Font::try_from_bytes(font_data).unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(AppState {
                token: token.clone(),
                font: font.clone(),
            }))
            .service(index)
    })
    .bind("0.0.0.0:80")?
    .run()
    .await
}
