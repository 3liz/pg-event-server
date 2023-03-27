//
// Landing page at root
//
use actix_web::{HttpRequest, HttpResponse, Result};

pub async fn handler(req: HttpRequest) -> Result<HttpResponse> {
    let url = req.url_for_static("landing_page")?;
    Ok(HttpResponse::Ok().body(url.to_string()))
}
