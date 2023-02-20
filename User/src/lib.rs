use std::{collections::hash_map::RandomState, env, error::Error, future, net::Ipv6Addr};

use argon2::Argon2;
use axum::{
    routing::{get, post},
    Router,
};
use bb8_bolt::{
    bolt_client,
    bolt_proto::version::{V4_3, V4_4},
};
use log::{info, warn};
use proto::{auth_service_client::AuthServiceClient, user_service_client::UserServiceClient};
use tokio::signal::unix::{signal, SignalKind};

pub mod proto;
mod user_service;

type DynError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct SharedState {
    bolt_pool: bb8::Pool<bb8_bolt::Manager>,
    auth_client: AuthServiceClient<tonic::transport::Channel>,
    user_client: UserServiceClient<tonic::transport::Channel>,
    argon2: Argon2<'static>,
    hash_builder: RandomState,
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
            bolt_pool,
            auth_client,
            user_client,
            argon2: Argon2::default(),
            hash_builder: RandomState::new(),
        })
        .route(
            "/health_check",
            get(|| future::ready(hyper::StatusCode::NO_CONTENT)),
        );

    let mut sigterm = signal(SignalKind::terminate())?;

    hyper::Server::bind(&(Ipv6Addr::UNSPECIFIED, 14514).into())
        .serve(root_router.into_make_service())
        .with_graceful_shutdown(async {
            match sigterm.recv().await {
                Some(()) => info!("start graceful shutdown"),
                None => warn!("stream of SIGTERM closed"),
            }
        })
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

#[cfg(test)]
mod t {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher,
    };
    #[test]
    fn hash_len_eq_32() {
        let salt = SaltString::generate(&mut OsRng);
        let password = "1".repeat(32);
        let argon2 = Argon2::default();

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        assert_eq!(hash.len(), 96, "{hash} len: {}", hash.len());

        let salt = SaltString::generate(&mut OsRng);
        let password = "114514";
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();

        assert_eq!(hash.len(), 96, "{hash} len: {}", hash.len());
    }
}
