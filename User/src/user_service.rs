use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use log::error;
use serde::{Deserialize, Serialize};

use crate::{
    proto::{token_response::TokenStatusCode, TokenRequest},
    SharedState,
};

#[derive(Deserialize)]
pub(crate) struct LoginReq {
    username: String,
    password: String,
}

#[derive(Serialize, Default)]
pub(crate) struct LoginRes {
    status_code: i32,
    status_msg: String,
    user_id: u32,
    token: String,
}

pub(crate) async fn login(
    State(mut conns): State<SharedState>,
    Query(q): Query<LoginReq>,
) -> Json<LoginRes> {
    match conns
        .auth_client
        .retrive_token(TokenRequest {
            username: q.username,
            password: q.password,
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            match res.status_code() {
                TokenStatusCode::Success => Json(LoginRes {
                    status_code: 0,
                    status_msg: "Sucess".into(),
                    user_id: res.user_id,
                    token: res.token,
                }),
                TokenStatusCode::Fail | TokenStatusCode::Unspecified => Json(LoginRes {
                    status_code: 403,
                    status_msg: "Forbidden".into(),
                    ..Default::default()
                }),
            }
        }
        Err(e) => {
            error!("{e}");
            Json(LoginRes {
                status_code: 502,
                status_msg: "Bad Gateway".into(),
                ..Default::default()
            })
        }
    }
}

pub(crate) async fn register(State(conns): State<SharedState>) -> impl IntoResponse {
    todo!()
}

pub(crate) async fn info(State(conns): State<SharedState>) -> impl IntoResponse {
    todo!()
}
