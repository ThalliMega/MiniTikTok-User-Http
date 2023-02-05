use std::net::{IpAddr, SocketAddr};

use axum::body::HttpBody;
use hyper::{
    body::{self, Buf},
    client::connect::Connect,
    Client, Uri,
};
use serde_json::Value;

use crate::DynError;

pub async fn service_url<C, B>(
    request_url: Uri,
    http_client: &Client<C, B>,
) -> Result<SocketAddr, DynError>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: HttpBody + Send + 'static + Default,
    B::Data: Send,
    B::Error: Into<DynError>,
{
    let res = http_client.get(request_url).await?;
    let body = body::aggregate(res).await?;

    let body: Value = serde_json::from_reader(body.reader())?;
    let body_err = "invalid consul response";

    let inner_body = body.get(0).ok_or(body_err)?;

    Ok((
        inner_body
            .get("ServiceAddress")
            .ok_or(body_err)?
            .as_str()
            .ok_or(body_err)?
            .parse::<IpAddr>()
            .map_err(|_| body_err)?,
        inner_body
            .get("ServicePort")
            .ok_or(body_err)?
            .as_u64()
            .ok_or(body_err)?
            .try_into()?,
    )
        .into())
}
