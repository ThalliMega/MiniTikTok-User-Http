use bb8_bolt::{bolt_client, bolt_proto};
use bb8_postgres::tokio_postgres::{self, error::SqlState};
use futures_io::{AsyncRead, AsyncWrite};
use log::{error, warn};

use crate::user_service::{LoginReq, LoginRes};

pub(crate) async fn postgres_regist(
    user_info: LoginReq,
    postgres_client: &tokio_postgres::Client,
) -> Result<u32, LoginRes> {
    match postgres_client
        .execute(
            "INSERT INTO auth (username, password) VALUES ($1, $2)",
            &[&user_info.username, &user_info.password],
        )
        .await
    {
        Ok(res) => {
            if res == 1 {
                retrieve_id(user_info.username, postgres_client).await
            } else if res == 0 {
                Err(LoginRes {
                    status_code: 403,
                    status_msg: "The username has been occupied.",
                    ..Default::default()
                })
            } else {
                error!("A insert into the auth table results in multiple row changes");
                Err(LoginRes {
                    status_code: 500,
                    status_msg: "Internal Server Error",
                    ..Default::default()
                })
            }
        }
        Err(e) => match e.as_db_error() {
            Some(e) if e.code() == &SqlState::UNIQUE_VIOLATION => Err(LoginRes {
                status_code: 403,
                status_msg: "username occupied",
                ..Default::default()
            }),
            _ => {
                error!("{e}");
                Err(LoginRes {
                    status_code: 502,
                    status_msg: "Bad Gateway",
                    ..Default::default()
                })
            }
        },
    }
}

pub(crate) async fn retrieve_id(
    username: String,
    postgres: &tokio_postgres::Client,
) -> Result<u32, LoginRes> {
    match postgres
        .query_opt("SELECT id FROM auth WHERE username = $1", &[&username])
        .await
    {
        Ok(Some(row)) => {
            // TODO: may panic?
            Ok(row.get(0))
        }
        Ok(None) => {
            error!("A user just created cannot be found");
            Err(LoginRes {
                status_code: 500,
                status_msg: "Internal Server Error",
                ..Default::default()
            })
        }
        Err(e) => {
            error!("{e}");
            Err(LoginRes {
                status_code: 502,
                status_msg: "Bad Gateway",
                ..Default::default()
            })
        }
    }
}

pub(crate) async fn bolt_regist<S: AsyncRead + AsyncWrite + Unpin>(
    username: String,
    user_id: u32,
    bolt_client: &mut bolt_client::Client<S>,
) {
    match bolt_client
        .run(
            "CREATE (:User {id: $user_id, username: $username});",
            Some(
                [
                    ("user_id", bolt_proto::Value::Integer(user_id.into())),
                    ("username", bolt_proto::Value::String(username)),
                ]
                .into_iter()
                .collect(),
            ),
            None,
        )
        .await
    {
        Ok(bolt_proto::Message::Success(_)) => {
            if let Err(e) = bolt_client.discard(None).await {
                warn!("{e}");
            }
            return;
        }
        Ok(_) => {}
        Err(e) => {
            error!("{e}");
        }
    }
    error!("Creation of the user {user_id} in graph db has failed.");
}
