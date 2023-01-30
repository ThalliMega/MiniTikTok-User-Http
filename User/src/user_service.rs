use axum::{extract::State, response::IntoResponse, Json};

use crate::SharedState;

pub(crate) async fn login(State(conns): State<SharedState>) -> impl IntoResponse {
    todo!()
}

pub(crate) async fn register(State(conns): State<SharedState>) -> impl IntoResponse {
    todo!()
}

pub(crate) async fn info(State(conns): State<SharedState>) -> impl IntoResponse {
    todo!()
}
