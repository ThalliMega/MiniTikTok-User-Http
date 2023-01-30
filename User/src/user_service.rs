use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use log::{error, warn};
use serde::{Deserialize, Serialize};

use crate::{
    proto::{
        token_response::TokenStatusCode, user_info_response::UserInfoStatusCode, TokenRequest,
        UserInfoRequest,
    },
    SharedState,
};

#[derive(Deserialize)]
pub(super) struct LoginReq {
    username: String,
    password: String,
}

#[derive(Serialize, Default)]
pub(super) struct LoginRes {
    status_code: i32,
    status_msg: String,
    user_id: u32,
    token: String,
}

pub(super) async fn login(
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

pub(super) async fn register(State(conns): State<SharedState>) -> impl IntoResponse {
    todo!()
}

#[derive(Deserialize)]
pub(super) struct InfoReq {
    user_id: u32,
    token: String,
}

#[derive(Serialize)]
pub(super) struct InfoRes {
    status_code: i32,
    status_msg: String,
    user: Option<UserInfo>,
}

#[derive(Serialize)]
struct UserInfo {
    id: u32,
    name: String,
    follow_count: u32,
    follower_count: u32,
    is_follow: bool,
}

pub(super) async fn info(
    State(mut conns): State<SharedState>,
    Query(q): Query<InfoReq>,
) -> Json<InfoRes> {
    match conns
        .user_client
        .get_info(UserInfoRequest {
            user_id: q.user_id,
            token: q.token,
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            match res.status_code() {
                UserInfoStatusCode::Success => Json(InfoRes {
                    status_code: 0,
                    status_msg: "Sucess".into(),
                    user: Some(UserInfo {
                        id: res.user_id,
                        name: res.username,
                        follow_count: res.follow_count,
                        follower_count: res.follower_count,
                        is_follow: res.is_follow,
                    }),
                }),
                UserInfoStatusCode::Unspecified => {
                    warn!("user info http service received an unspecified status");
                    Json(InfoRes {
                        status_code: 500,
                        status_msg: "Can't recognize the status code sent by gateway.".into(),
                        user: None,
                    })
                }
                UserInfoStatusCode::AuthFail => Json(InfoRes {
                    status_code: 401,
                    status_msg: "Unauthorized".into(),
                    user: None,
                }),
                UserInfoStatusCode::TargetNotFound => Json(InfoRes {
                    status_code: 404,
                    status_msg: "Not Found".into(),
                    user: None,
                }),
            }
        }
        Err(e) => {
            error!("{e}");
            Json(InfoRes {
                status_code: 502,
                status_msg: "Bad Gateway".into(),
                user: None,
            })
        }
    }
}
