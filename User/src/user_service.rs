use crate::AuthServiceClient;
use axum::{
    extract::{Query, State},
    Json,
};
use log::{error, warn};
use serde::{Deserialize, Serialize};

use crate::{
    proto::{
        auth_response::AuthStatusCode, token_response::TokenStatusCode,
        user_info_response::UserInfoStatusCode, AuthRequest, TokenRequest, UserInfoRequest,
    },
    user_regist::{bolt_regist, postgres_regist},
    SharedState,
};

#[derive(Deserialize, Clone)]
pub(crate) struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Default)]
pub(crate) struct LoginRes {
    pub status_code: i32,
    pub status_msg: &'static str,
    pub user_id: u32,
    pub token: String,
}

async fn real_login(
    req: LoginReq,
    auth_client: &mut AuthServiceClient<tonic::transport::Channel>,
) -> Json<LoginRes> {
    match auth_client
        .retrive_token(TokenRequest {
            username: req.username,
            password: req.password,
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            match res.status_code() {
                TokenStatusCode::Success => Json(LoginRes {
                    status_code: 0,
                    status_msg: "Sucess",
                    user_id: res.user_id,
                    token: res.token,
                }),
                TokenStatusCode::Fail | TokenStatusCode::Unspecified => Json(LoginRes {
                    status_code: 403,
                    status_msg: "Forbidden",
                    ..Default::default()
                }),
            }
        }
        Err(e) => {
            error!("{e}");
            Json(LoginRes {
                status_code: 502,
                status_msg: "Bad Gateway",
                ..Default::default()
            })
        }
    }
}

pub(crate) async fn login(
    State(mut conns): State<SharedState>,
    Query(q): Query<LoginReq>,
) -> Json<LoginRes> {
    real_login(q, &mut conns.auth_client).await
}

pub(crate) async fn register(
    State(mut conns): State<SharedState>,
    Query(q): Query<LoginReq>,
) -> Json<LoginRes> {
    let bad_gateway = Json(LoginRes {
        status_code: 502,
        status_msg: "Bad Gateway",
        ..Default::default()
    });

    let postgres_client = match conns.postgres_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Connect to Postgres failed: {e}");
            return bad_gateway;
        }
    };

    let user_id = match postgres_regist(q.clone(), &postgres_client).await {
        Err(e) => return Json(e),
        Ok(id) => id,
    };

    let mut bolt_client = if let Ok(conn) = conns.bolt_pool.get().await {
        conn
    } else {
        return bad_gateway;
    };
    // TODO: uncessary clone?
    bolt_regist(q.username.clone(), user_id, &mut bolt_client).await;

    real_login(q, &mut conns.auth_client).await
}

#[derive(Deserialize)]
pub(super) struct InfoReq {
    user_id: u32,
    token: String,
}

#[derive(Serialize)]
pub(super) struct InfoRes {
    status_code: i32,
    status_msg: &'static str,
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
    let user_id = match auth(q.token, conns.auth_client).await {
        Some(id) => id,
        None => {
            return Json(InfoRes {
                status_code: 401,
                status_msg: "Unauthorized",
                user: None,
            })
        }
    };

    match conns
        .user_client
        .get_info(UserInfoRequest {
            target_id: q.user_id,
            self_id: user_id,
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            match res.status_code() {
                UserInfoStatusCode::Success => Json(InfoRes {
                    status_code: 0,
                    status_msg: "Sucess",
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
                        status_msg: "Can't recognize the status code sent by gateway.",
                        user: None,
                    })
                }
                UserInfoStatusCode::TargetNotFound => Json(InfoRes {
                    status_code: 404,
                    status_msg: "Not Found",
                    user: None,
                }),
            }
        }
        Err(e) => {
            error!("{e}");
            Json(InfoRes {
                status_code: 502,
                status_msg: "Bad Gateway",
                user: None,
            })
        }
    }
}

async fn auth(
    token: String,
    mut auth_client: AuthServiceClient<tonic::transport::Channel>,
) -> Option<u32> {
    match auth_client
        .auth(AuthRequest { token })
        .await
        .map(|r| r.into_inner())
    {
        Ok(res) if res.status_code() == AuthStatusCode::Success => res.user_id.into(),
        Err(e) => {
            error!("{e}");
            None
        }
        _ => None,
    }
}
