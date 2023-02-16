use std::{env, error::Error, future, net::Ipv6Addr};

use axum::{
    routing::{get, post},
    Router,
};
use bb8_bolt::{
    bolt_client,
    bolt_proto::version::{V4_3, V4_4},
};
use bb8_postgres::tokio_postgres::NoTls;
use proto::{auth_service_client::AuthServiceClient, user_service_client::UserServiceClient};

pub mod proto;
mod user_regist;
mod user_service;

type DynError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct SharedState {
    postgres_pool: bb8::Pool<bb8_postgres::PostgresConnectionManager<NoTls>>,
    bolt_pool: bb8::Pool<bb8_bolt::Manager>,
    auth_client: AuthServiceClient<tonic::transport::Channel>,
    user_client: UserServiceClient<tonic::transport::Channel>,
}

/// This function will initialize the [env-logger](https://docs.rs/env_logger) and start the server.  
/// Because this function will be used in integration tests,
/// it will **NOT** block the main thread.
///
/// # Panics
///
/// Panics if called from **outside** of the Tokio runtime.
pub async fn start_up() -> Result<(), DynError> {
    env_logger::init();

    let bolt_metadata: bolt_client::Metadata = [
        ("user_agent", "MiniTikTok-User-Http/0"),
        ("scheme", "basic"),
        (
            "principal",
            // TODO: String::leak
            Box::leak(get_env_var("BOLT_USERNAME")?.into_boxed_str()),
        ),
        (
            "credentials",
            // TODO: String::leak
            Box::leak(get_env_var("BOLT_PASSWORD")?.into_boxed_str()),
        ),
    ]
    .into_iter()
    .collect();

    let bolt_url = get_env_var("BOLT_URL")?;

    let bolt_domain = env::var("BOLT_DOMAIN").ok();

    let auth_url = get_env_var("AUTH_URL")?;

    let user_url = get_env_var("USER_URL")?;

    let postgres_url = get_env_var("POSTGRES_URL")?;

    let postgres_config = postgres_url.parse()?;

    let postgres_manager = bb8_postgres::PostgresConnectionManager::new(postgres_config, NoTls);

    let postgres_pool = bb8::Pool::builder().build(postgres_manager).await?;

    let bolt_manager =
        bb8_bolt::Manager::new(bolt_url, bolt_domain, [V4_4, V4_3, 0, 0], bolt_metadata).await?;

    let auth_client = AuthServiceClient::connect(auth_url).await?;

    let user_client = UserServiceClient::connect(user_url).await?;

    let bolt_pool = bb8::Pool::builder().build(bolt_manager).await?;

    let router = Router::new()
        .route("/register/", post(user_service::register))
        .route("/login/", post(user_service::login))
        .route("/", get(user_service::info));

    let root_router = Router::new()
        .nest("/douyin/user", router)
        .with_state(SharedState {
            postgres_pool,
            bolt_pool,
            auth_client,
            user_client,
        })
        .route(
            "/health_check",
            get(|| future::ready(hyper::StatusCode::NO_CONTENT)),
        );

    hyper::Server::bind(&(Ipv6Addr::UNSPECIFIED, 14514).into())
        .serve(root_router.into_make_service())
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
        .await?;

    Ok(())
}

fn get_env_var(s: &str) -> Result<String, String> {
    env::var(s).map_err(|_| format!("{s} doesn't exist"))
}

/// Build a runtime and block on a `Future`.
pub fn block_on<F: std::future::Future>(f: F) -> Result<F::Output, std::io::Error> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(f))
}
