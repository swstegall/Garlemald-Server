// garlemald-server — Rust port of a FINAL FANTASY XIV v1.23b server emulator (lobby/world/map)
// Copyright (C) 2026  Samuel Stegall
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

//! HTTP handlers for the login / signup flow. Each successful submission
//! mints a fresh 56-char session id, inserts it into the `sessions` table,
//! and 302-redirects the client to `ffxiv://login_success?sessionId=…`.
//! The `garlemald-client` webview (`src/login/webview.rs`) intercepts that
//! scheme and hands the id off to the launch pipeline.

use std::sync::Arc;

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::extract::{Form, Query, State};
use axum::response::{Html, IntoResponse, Redirect, Response};
use rand::RngCore;
use serde::Deserialize;

use crate::database::Database;
use crate::session;
use crate::templates::{login_page, signup_page};

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub session_hours: u32,
}

const USERNAME_MIN: usize = 3;
const USERNAME_MAX: usize = 64;
const PASSWORD_MIN: usize = 8;
const PASSWORD_MAX: usize = 128;
const EMAIL_MAX: usize = 254;

// ---------------------------------------------------------------------------
// GET /  — redirect to /login so the webview lands on a form either way.
// ---------------------------------------------------------------------------

pub async fn root() -> Redirect {
    Redirect::to("/login")
}

// ---------------------------------------------------------------------------
// GET /login
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct LoginQuery {
    #[serde(default)]
    pub err: Option<String>,
    #[serde(default)]
    pub info: Option<String>,
    #[serde(default)]
    pub u: Option<String>,
}

pub async fn login_form(Query(q): Query<LoginQuery>) -> Html<String> {
    Html(login_page(
        q.err.as_deref(),
        q.info.as_deref(),
        q.u.as_deref().unwrap_or(""),
    ))
}

// ---------------------------------------------------------------------------
// POST /login
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_submit(State(state): State<AppState>, Form(form): Form<LoginForm>) -> Response {
    let username = form.username.trim();
    let password = form.password;

    if username.is_empty() || password.is_empty() {
        return redirect_login("Username and password are required.", username);
    }

    let stored = match state.db.find_user_by_username(username).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            tracing::debug!(username, "login: unknown user");
            return redirect_login("Invalid username or password.", username);
        }
        Err(e) => {
            tracing::error!(error = %e, "login: db lookup failed");
            return redirect_login("Server error, please try again.", username);
        }
    };

    if !verify_password(&password, &stored.password_hash) {
        tracing::debug!(username, "login: bad password");
        return redirect_login("Invalid username or password.", username);
    }

    match mint_session(&state, stored.id).await {
        Ok(sid) => {
            tracing::info!(user_id = stored.id, "login: session minted");
            success_redirect(&sid)
        }
        Err(e) => {
            tracing::error!(error = %e, "login: session insert failed");
            redirect_login("Server error, please try again.", username)
        }
    }
}

// ---------------------------------------------------------------------------
// GET /signup
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct SignupQuery {
    #[serde(default)]
    pub err: Option<String>,
    #[serde(default)]
    pub u: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
}

pub async fn signup_form(Query(q): Query<SignupQuery>) -> Html<String> {
    Html(signup_page(
        q.err.as_deref(),
        q.u.as_deref().unwrap_or(""),
        q.e.as_deref().unwrap_or(""),
    ))
}

// ---------------------------------------------------------------------------
// POST /signup
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SignupForm {
    pub username: String,
    pub password: String,
    pub confirm: String,
    #[serde(default)]
    pub email: String,
}

pub async fn signup_submit(
    State(state): State<AppState>,
    Form(form): Form<SignupForm>,
) -> Response {
    let username = form.username.trim().to_string();
    let email = form.email.trim().to_string();

    if let Err(msg) = validate_username(&username) {
        return redirect_signup(msg, &username, &email);
    }
    if form.password.len() < PASSWORD_MIN {
        return redirect_signup("Password must be at least 8 characters.", &username, &email);
    }
    if form.password.len() > PASSWORD_MAX {
        return redirect_signup("Password is too long.", &username, &email);
    }
    if form.password != form.confirm {
        return redirect_signup("Passwords do not match.", &username, &email);
    }
    if !email.is_empty() && (email.len() > EMAIL_MAX || !email.contains('@')) {
        return redirect_signup("That email doesn't look valid.", &username, &email);
    }

    let hash = match hash_password(&form.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "signup: argon2 hash failed");
            return redirect_signup("Server error, please try again.", &username, &email);
        }
    };

    let email_opt = if email.is_empty() {
        None
    } else {
        Some(email.as_str())
    };
    let user_id = match state.db.create_user(&username, &hash, email_opt).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            return redirect_signup("That username is already taken.", &username, &email);
        }
        Err(e) => {
            tracing::error!(error = %e, "signup: db insert failed");
            return redirect_signup("Server error, please try again.", &username, &email);
        }
    };

    match mint_session(&state, user_id).await {
        Ok(sid) => {
            tracing::info!(user_id, "signup: account created, session minted");
            success_redirect(&sid)
        }
        Err(e) => {
            tracing::error!(error = %e, user_id, "signup: session insert failed");
            // The user row is live; point them at the login page so they
            // can re-auth once the transient failure clears.
            let msg = format!("Account created but sign-in failed: {e}. Please log in.");
            Redirect::to(&format!("/login?u={}&err={}", pct(&username), pct(&msg),)).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// GET /healthz — tiny liveness check for orchestrators / run-all.sh smoke
// tests. Returns plain text so curl output is readable.
// ---------------------------------------------------------------------------

pub async fn healthz() -> &'static str {
    "ok"
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

async fn mint_session(state: &AppState, user_id: i64) -> anyhow::Result<String> {
    let sid = session::generate();
    state
        .db
        .insert_session(&sid, user_id, state.session_hours)
        .await?;
    Ok(sid)
}

fn success_redirect(session_id: &str) -> Response {
    // Custom scheme — `Redirect::to` would happily emit it too, but we build
    // the URL explicitly so it's obvious at the call site what contract
    // we're honouring (see `garlemald-client/src/login/webview.rs`).
    let url = format!("ffxiv://login_success?sessionId={session_id}");
    Redirect::to(&url).into_response()
}

fn redirect_login(err: &str, username: &str) -> Response {
    Redirect::to(&format!("/login?u={}&err={}", pct(username), pct(err))).into_response()
}

fn redirect_signup(err: &str, username: &str, email: &str) -> Response {
    Redirect::to(&format!(
        "/signup?u={}&e={}&err={}",
        pct(username),
        pct(email),
        pct(err),
    ))
    .into_response()
}

fn validate_username(u: &str) -> Result<(), &'static str> {
    if u.len() < USERNAME_MIN {
        return Err("Username must be at least 3 characters.");
    }
    if u.len() > USERNAME_MAX {
        return Err("Username is too long.");
    }
    let ok = u
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if !ok {
        return Err("Username may only contain letters, numbers, . _ and -");
    }
    Ok(())
}

fn hash_password(pw: &str) -> Result<String, argon2::password_hash::Error> {
    // password_hash's OsRng ships behind an off-by-default feature; pulling
    // it in would mean re-enabling `rand_core/getrandom` globally, so we
    // take 16 random bytes from our own `rand` crate and base64-encode them
    // into the `SaltString` format argon2 expects.
    let mut raw = [0u8; 16];
    rand::rng().fill_bytes(&mut raw);
    let salt = SaltString::encode_b64(&raw)?;
    Ok(Argon2::default()
        .hash_password(pw.as_bytes(), &salt)?
        .to_string())
}

fn verify_password(pw: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "stored hash is not a valid argon2 string");
            return false;
        }
    };
    Argon2::default()
        .verify_password(pw.as_bytes(), &parsed)
        .is_ok()
}

fn pct(s: &str) -> String {
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_username_rejects_short() {
        assert!(validate_username("ab").is_err());
    }

    #[test]
    fn validate_username_rejects_spaces() {
        assert!(validate_username("bad name").is_err());
    }

    #[test]
    fn validate_username_allows_punct() {
        assert!(validate_username("sam.s-99_t").is_ok());
    }

    #[test]
    fn hash_roundtrips() {
        let h = hash_password("hunter2hunter2").unwrap();
        assert!(verify_password("hunter2hunter2", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn pct_encodes_specials() {
        assert_eq!(pct("a b"), "a%20b");
        assert_eq!(pct("a&b"), "a%26b");
    }
}
