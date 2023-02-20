use std::hash::{BuildHasher, Hash, Hasher};

use crate::AuthServiceClient;
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    PasswordHasher,
};
use axum::{
    extract::{Query, State},
    Json,
};
use bb8_bolt::bolt_proto;
use log::{error, warn};
use serde::{Deserialize, Serialize};

use crate::{
    proto::{auth_response::AuthStatusCode, token_response::TokenStatusCode, *},
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
    pub user_id: i64,
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
    if q.username.len() > 32 {
        return Json(LoginRes {
            status_code: 400,
            status_msg: "username too long: max length is 32 bytes",
            ..Default::default()
        });
    }

    if q.password.len() > 32 {
        return Json(LoginRes {
            status_code: 400,
            status_msg: "password too long: max length is 32 bytes",
            ..Default::default()
        });
    }

    let bad_gateway = Json(LoginRes {
        status_code: 502,
        status_msg: "Bad Gateway",
        ..Default::default()
    });

    let mut conn = match conns.bolt_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Connect to graph db failed: {e}");
            return bad_gateway;
        }
    };

    let mut hasher = conns.hash_builder.build_hasher();
    q.username.hash(&mut hasher);
    let hash_name = hasher.finish() as i64;

    let salt = SaltString::generate(OsRng);
    let passwd_h = match conns.argon2.hash_password(q.password.as_bytes(), &salt) {
        Ok(hash) => hash.to_string(),
        Err(e) => {
            error!("{e}");
            return Json(LoginRes {
                status_code: 500,
                status_msg: "Internal Server Error",
                ..Default::default()
            });
        }
    };

    match conn.run("CREATE (:User { id: $id, username: $username, password_hash: $passwd_h, avatar: \"\", background_image: \"\", signature: \"\" });", Some([("id", bolt_proto::value::Value::Integer(hash_name)), ("username", q.username.clone().into()), ("passwd_h", passwd_h.into())].into_iter().collect()), None).await {
        Ok(bolt_proto::Message::Success(_)) => {}
        Ok(bolt_proto::Message::Failure(f)) if matches!(f.metadata().get("code"), Some(bolt_proto::Value::String(s)) if s == "Neo.ClientError.Schema.ConstraintValidationFailed") => {
            // TODO: discard result
            _ = conn.reset().await;
            return Json(LoginRes {
                status_code: 403,
                status_msg: "The username may be occupied.",
                ..Default::default()
            });
        }
        Ok(e) => {
            warn!("{e:?}");
            // TODO: discard result
            _ = conn.reset().await;
            return bad_gateway;
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    }

    match conn.discard(Some([("n", -1)].into_iter().collect())).await {
        Ok(bolt_proto::Message::Success(_)) => {}
        Ok(e) => {
            warn!("{e:?}");
            // TODO: discard result
            _ = conn.reset().await;
            return bad_gateway;
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    }

    real_login(q, &mut conns.auth_client).await
}

#[derive(Deserialize)]
pub(super) struct InfoReq {
    user_id: i64,
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
    id: i64,
    name: String,
    follow_count: i64,
    follower_count: i64,
    is_follow: bool,
    avatar: String,
    background_image: String,
    signature: String,
    total_favorited: i64,
    work_count: i64,
    favorite_count: i64,
}

pub(super) async fn info(
    State(mut conns): State<SharedState>,
    Query(q): Query<InfoReq>,
) -> Json<InfoRes> {
    let bad_gateway = Json(InfoRes {
        status_code: 502,
        status_msg: "Bad Gateway",
        user: None,
    });

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

    // TODO: refactor needed: parallel async
    let info = match conns
        .user_client
        .get_infos(UserIds {
            user_ids: [user_id].into(),
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            if let Some(res) = res.infos.get(0) {
                res.clone()
            } else {
                return Json(InfoRes {
                    status_code: 404,
                    status_msg: "Not Found",
                    user: None,
                });
            }
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    let follow_count = match conns
        .user_client
        .get_follow_counts(UserIds {
            user_ids: [user_id].into(),
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            if let Some(res) = res.responses.get(0) {
                res.value
            } else {
                error!("no follow count");
                0
            }
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    let follower_count = match conns
        .user_client
        .get_follower_counts(UserIds {
            user_ids: [user_id].into(),
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            if let Some(res) = res.responses.get(0) {
                res.value
            } else {
                error!("no follower count");
                0
            }
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    let favorite_count = match conns
        .user_client
        .get_favorite_counts(UserIds {
            user_ids: [user_id].into(),
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            if let Some(res) = res.responses.get(0) {
                res.value
            } else {
                error!("no favorite count");
                0
            }
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    let total_favorited = match conns
        .user_client
        .get_total_favoriteds(UserIds {
            user_ids: [user_id].into(),
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            if let Some(res) = res.responses.get(0) {
                res.value
            } else {
                error!("no total favorited");
                0
            }
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    let work_count = match conns
        .user_client
        .get_work_counts(UserIds {
            user_ids: [user_id].into(),
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            if let Some(res) = res.responses.get(0) {
                res.value
            } else {
                error!("no work count");
                0
            }
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    let is_follow = match conns
        .user_client
        .check_follows(FollowCheckRequests {
            self_id: user_id,
            target_ids: vec![q.user_id],
        })
        .await
    {
        Ok(r) => {
            let res = r.into_inner();
            // Believe that db not broken; id == target_id omitted
            res.target_ids.get(0).is_some()
        }
        Err(e) => {
            error!("{e}");
            return bad_gateway;
        }
    };

    Json(InfoRes {
        status_code: 0,
        status_msg: "Success",
        user: Some(UserInfo {
            id: info.id,
            name: info.username,
            follow_count,
            follower_count,
            is_follow,
            avatar: info.avatar,
            background_image: info.background_img,
            signature: info.signature,
            total_favorited,
            work_count,
            favorite_count,
        }),
    })
}

async fn auth(
    token: String,
    mut auth_client: AuthServiceClient<tonic::transport::Channel>,
) -> Option<i64> {
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
